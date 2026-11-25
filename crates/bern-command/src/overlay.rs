//! Text that floats over everything else on the screen, such as desktop
//! lyrics or a line on the taskbar: a few lines, each with its size, weight
//! and colour, drawn with a soft shadow so they read on any background, on
//! a transparent window above the others.
//!
//! [`show`] puts up or changes the overlay of an id, [`hide`] takes it down.
//! Locked, clicks go through it to what lies under it; unlocked, a faint
//! plate shows behind the text and it can be dragged. Where it was left is
//! kept for the next time. [`Place::Taskbar`] stands it on the Windows
//! taskbar, beside the notification area, in the room the taskbar's own
//! buttons leave; elsewhere it is a free overlay.
//!
//! A free overlay can carry a bar of [`Button`]s above its lines, which
//! shows while the pointer is over it (on Android, where nothing hovers, a
//! tap shows and hides it). On a locked overlay only the buttons marked
//! [`Button::while_locked`] show, and only they take the pointer, so the
//! user can unlock it where it is. Presses come back through
//! [`take_presses`], each waking the engine.
//!
//! macOS draws it on a borderless panel, Windows on a layered window,
//! Android on a system overlay once the user allows one ([`allowed`],
//! [`ask`]), Linux through the GTK host, which installs itself with
//! [`install`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

/// The treatment of overlay text, including annotations and the lit region.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextEffect {
    None,
    #[default]
    Shadow,
    Outline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub effect: TextEffect,
    /// Multiplies the text's original alpha without affecting overlay controls.
    pub opacity: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            effect: TextEffect::Shadow,
            opacity: 1.0,
        }
    }
}

static RECTS: Mutex<BTreeMap<String, (f64, f64, f64, f64)>> = Mutex::new(BTreeMap::new());
type SyncHook = Arc<dyn Fn(&str) + Send + Sync>;
static SYNC_HOOK: RwLock<Option<SyncHook>> = RwLock::new(None);

/// Current screen rectangle `(x, y, width, height)` of an unlocked free overlay,
/// in native platform screen coordinates (Win32 physical px on Windows, AppKit
/// screen points on macOS). `None` when hidden, locked, or taskbar-anchored.
pub fn screen_rect(id: &str) -> Option<(f64, f64, f64, f64)> {
    RECTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .copied()
}

/// Register a callback invoked when an active free overlay locks, hides, or
/// moves to the taskbar.
pub fn set_sync_hook(hook: impl Fn(&str) + Send + Sync + 'static) {
    *SYNC_HOOK.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(hook));
}

pub(crate) fn update_rect(id: &str, rect: Option<(f64, f64, f64, f64)>) {
    let deactivated = {
        let mut rects = RECTS.lock().unwrap_or_else(|e| e.into_inner());
        match rect {
            Some(r) => {
                rects.insert(id.to_string(), r);
                false
            }
            None => rects.remove(id).is_some(),
        }
    };
    if deactivated && let Some(hook) = SYNC_HOOK.read().unwrap_or_else(|e| e.into_inner()).clone() {
        hook(id);
    }
}

/// One line of an overlay.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    /// JSON `[[start, end, "reading"]]`, in half-open Unicode scalar ranges.
    /// Empty means no annotations. Invalid and overlapping spans are ignored.
    pub ruby: String,
    /// In points.
    pub size: f64,
    pub bold: bool,
    /// Red, green, blue and alpha.
    pub color: [u8; 4],
    /// As far through its characters as this in `color`, the rest faded:
    /// 2.5 is two and the left half of the third. All of it when there is no
    /// such place, or it is below zero.
    pub lit: Option<f64>,
    /// Lit along this by itself instead, every frame, with nothing more to
    /// show until the route or its clock changes.
    pub route: Option<Route>,
}

/// How far a line is lit as a clock runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Route {
    /// Seconds and characters, in order: the place moves evenly from each
    /// to the next, and stays at the first before it and the last after it.
    pub points: Vec<(f64, f64)>,
    /// The second the clock showed.
    pub clock: f64,
    /// When it showed that, in seconds since the Unix epoch; zero while the
    /// clock stands still.
    pub since: f64,
}

impl Route {
    /// From `path` as `Text(lit_path:)` takes it: seconds and characters
    /// apart by spaces.
    pub fn new(path: &str, clock: f64, since: f64) -> Self {
        let numbers: Vec<f64> = path
            .split_ascii_whitespace()
            .filter_map(|number| number.parse().ok())
            .collect();
        let (pairs, _) = numbers.as_chunks::<2>();
        let points = pairs.iter().map(|pair| (pair[0], pair[1])).collect();
        Self {
            points,
            clock,
            since,
        }
    }

    /// As `Route::new` reads it.
    pub fn path(&self) -> String {
        let points: Vec<String> = self
            .points
            .iter()
            .map(|(second, place)| format!("{second} {place}"))
            .collect();
        points.join(" ")
    }

    /// The clock runs, along a route with somewhere to go.
    pub fn moving(&self) -> bool {
        self.since > 0.0 && !self.points.is_empty()
    }

    /// The second the clock shows now.
    pub fn second(&self) -> f64 {
        if self.since <= 0.0 {
            return self.clock;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |since| since.as_secs_f64());
        self.clock + now - self.since
    }

    /// Where it stands at `second`.
    pub fn at(&self, second: f64) -> f64 {
        let Some((first, last)) = self.points.first().zip(self.points.last()) else {
            return 0.0;
        };
        if second <= first.0 {
            return first.1;
        }
        self.points
            .windows(2)
            .find(|pair| second < pair[1].0)
            .map_or(last.1, |pair| {
                let ((from, start), (to, end)) = (pair[0], pair[1]);
                start + (end - start) * (second - from) / (to - from)
            })
    }

    /// Where it stands now.
    pub fn place(&self) -> f64 {
        self.at(self.second())
    }

    /// The seconds and places it passes from `second` on, one at each whole
    /// character too: characters are not all as wide.
    #[cfg(any(target_os = "macos", test))]
    fn steps(&self, second: f64) -> Vec<(f64, f64)> {
        let mut steps = vec![(second, self.at(second))];
        for pair in self.points.windows(2) {
            let ((from, start), (to, end)) = (pair[0], pair[1]);
            if to <= second {
                continue;
            }
            if from > second && steps.last() != Some(&(from, start)) {
                steps.push((from, start));
            }
            if end > start {
                let mut place = self.at(second.max(from)).floor() + 1.0;
                while place < end {
                    steps.push((from + (to - from) * (place - start) / (end - start), place));
                    place += 1.0;
                }
            }
            steps.push((to, end));
        }
        steps
    }
}

/// How much of `Line::color`'s alpha what `Line::lit` leaves out keeps.
pub const FADED: f64 = 0.4;

impl Line {
    /// Valid readings in source order; coordinates always refer to the
    /// original text, never UTF-8 bytes or UTF-16 code units.
    pub fn ruby_spans(&self) -> Vec<(usize, usize, String)> {
        let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&self.ruby) else {
            return Vec::new();
        };
        let count = self.text.chars().count();
        let mut spans: Vec<_> = values
            .into_iter()
            .filter_map(|value| {
                let row = value.as_array()?;
                if row.len() != 3 {
                    return None;
                }
                let start = usize::try_from(row[0].as_u64()?).ok()?;
                let end = usize::try_from(row[1].as_u64()?).ok()?;
                let reading = row[2].as_str()?;
                (start < end && end <= count && !reading.is_empty())
                    .then(|| (start, end, reading.to_string()))
            })
            .collect();
        spans.sort_by_key(|span| (span.0, span.1));
        let mut end = 0;
        spans.retain(|span| {
            if span.0 < end {
                return false;
            }
            end = span.1;
            true
        });
        spans
    }

    /// It is lit part of the way, or may come to be.
    pub fn lights(&self) -> bool {
        self.route.is_some() || self.lit.is_some_and(|lit| lit >= 0.0)
    }

    /// How far it is lit now, if it lights up at all.
    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    fn lit_now(&self) -> Option<f64> {
        match &self.route {
            Some(route) => Some(route.place()),
            None => self.lit.filter(|lit| *lit >= 0.0),
        }
    }
}

/// Where an overlay stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// Where the user last left it; at first low in the middle of the main
    /// screen.
    #[default]
    Free,
    /// On the Windows taskbar, beside the notification area, in the room
    /// its buttons, widgets and search box leave: a longer line ends in an
    /// ellipsis.
    Taskbar,
}

/// A button on an overlay's bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// What [`take_presses`] names it by.
    pub id: String,
    /// An SF Symbol name. macOS draws any; the other systems draw
    /// `backward.fill`, `forward.fill`, `play.fill`, `pause.fill`,
    /// `lock.fill`, `lock.open.fill` and `xmark`.
    pub symbol: String,
    /// Shown on a locked overlay too, where it alone takes the pointer.
    pub while_locked: bool,
}

/// What an overlay shows and how.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    /// From the top.
    pub lines: Vec<Line>,
    pub place: Place,
    /// Clicks go through it; unlocked, it shows a plate and can be dragged.
    pub locked: bool,
    /// The bar above the lines, from the left; none on the taskbar.
    pub buttons: Vec<Button>,
}

static PRESSES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// The buttons pressed since the last call, each as its overlay's id and
/// its own, the earliest first.
pub fn take_presses() -> Vec<(String, String)> {
    std::mem::take(&mut *PRESSES.lock().unwrap_or_else(|e| e.into_inner()))
}

/// Tell of a press on the button `button` of the overlay `overlay`: what a
/// host that draws overlays calls, and the systems' own windows here.
pub fn press(overlay: &str, button: &str) {
    PRESSES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((overlay.to_string(), button.to_string()));
    if let Some(waker) = crate::wake::waker() {
        waker.wake();
    }
}

/// A host that draws overlays itself (the GTK host on Linux).
pub trait Host: Send + Sync {
    fn show(&self, id: &str, overlay: &Overlay);
    fn show_styled(&self, id: &str, overlay: &Overlay, _style: TextStyle) {
        self.show(id, overlay);
    }
    /// Only how far the lines are lit changed since `show`.
    fn lit(&self, id: &str, overlay: &Overlay) {
        self.show(id, overlay);
    }
    /// Only line colors (and optionally how far they are lit) changed since `show_styled`.
    fn color(&self, id: &str, overlay: &Overlay, style: TextStyle) {
        self.show_styled(id, overlay, style);
    }
    fn hide(&self, id: &str);
}

static HOST: RwLock<Option<Arc<dyn Host>>> = RwLock::new(None);

/// Draw overlays through `host` from now on.
pub fn install(host: impl Host + 'static) {
    *HOST.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(host));
}

fn host() -> Option<Arc<dyn Host>> {
    HOST.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Whether this system shows overlays at all.
pub fn supported() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "android"
    )) || host().is_some()
}

/// Whether the user lets the application draw over other apps: always,
/// except on Android before the user allowed it.
pub fn allowed() -> bool {
    #[cfg(target_os = "android")]
    return crate::platform::request("overlay_allowed", "", "").is_ok_and(|answer| answer == "yes");
    #[cfg(not(target_os = "android"))]
    supported()
}

/// Open the system's page where the user allows drawing over other apps
/// (Android); nothing elsewhere.
pub fn ask() {
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_ask", "", "");
}

/// What each overlay shows now.
static SHOWN: Mutex<BTreeMap<String, (Overlay, TextStyle)>> = Mutex::new(BTreeMap::new());

/// What an overlay needs for what it shows next.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    Nothing,
    /// Only how far a line is lit, or the route it follows.
    Lit,
    /// Only line colors (and optionally lit progress/route) changed.
    Color,
    All,
}

impl Overlay {
    fn change_to(&self, next: &Overlay) -> Change {
        if self == next {
            return Change::Nothing;
        }
        // Text and ruby are layout, not timing: changing either must rebuild
        // the native surface even when only the clock also moved.
        let same_layout = |line: &Line, other: &Line| {
            Line {
                color: [0; 4],
                lit: None,
                route: None,
                ..line.clone()
            } == Line {
                color: [0; 4],
                lit: None,
                route: None,
                ..other.clone()
            } && line.lit.is_some() == other.lit.is_some()
                && line.route.is_some() == other.route.is_some()
        };
        let layout_unchanged = self.lines.len() == next.lines.len()
            && self
                .lines
                .iter()
                .zip(&next.lines)
                .all(|(a, b)| same_layout(a, b));
        let rest = Overlay {
            lines: Vec::new(),
            ..self.clone()
        } == Overlay {
            lines: Vec::new(),
            ..next.clone()
        };
        if layout_unchanged && rest {
            let same_color = self
                .lines
                .iter()
                .zip(&next.lines)
                .all(|(a, b)| a.color == b.color);
            if same_color {
                Change::Lit
            } else {
                Change::Color
            }
        } else {
            Change::All
        }
    }
}

/// Show `overlay` as the overlay `id`, putting it up when it is not.
pub fn show(id: &str, overlay: &Overlay) {
    show_styled(id, overlay, TextStyle::default());
}

/// Show `overlay` with explicit text effect and opacity.
pub fn show_styled(id: &str, overlay: &Overlay, style: TextStyle) {
    let style = TextStyle {
        effect: style.effect,
        opacity: if style.opacity.is_finite() {
            style.opacity.clamp(0.0, 1.0)
        } else {
            1.0
        },
    };
    let was = SHOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_string(), (overlay.clone(), style));
    let change = match was {
        None => Change::All,
        Some((was_overlay, was_style)) if was_style != style => {
            let _ = was_overlay;
            Change::All
        }
        Some((was_overlay, _)) => was_overlay.change_to(overlay),
    };
    if change == Change::All && (overlay.locked || overlay.place != Place::Free) {
        update_rect(id, None);
    }
    match (change, host()) {
        (Change::Nothing, _) => {}
        (Change::Lit, Some(host)) => host.lit(id, overlay),
        (Change::Color, Some(host)) => host.color(id, overlay, style),
        (Change::All, Some(host)) => host.show_styled(id, overlay, style),
        (Change::Lit, None) => lit_here(id, overlay, style),
        (Change::Color, None) => color_here(id, overlay, style),
        (Change::All, None) => show_here(id, overlay, style),
    }
}

/// Take the overlay `id` down.
pub fn hide(id: &str) {
    update_rect(id, None);
    SHOWN.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    match host() {
        Some(host) => host.hide(id),
        None => hide_here(id),
    }
}

/// Change how far an overlay of this system's own windows is lit.
fn lit_here(id: &str, overlay: &Overlay, style: TextStyle) {
    #[cfg(target_os = "macos")]
    mac::lit(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "windows")]
    win::lit(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_lit", id, &as_json(overlay, style));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, overlay, style);
}

/// Pending update queued for the Windows overlay thread.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Debug, PartialEq)]
enum WinOp {
    Show(String, Overlay, TextStyle),
    /// Only line colors (and optionally lit progress/route) changed.
    Color(String, Overlay, TextStyle),
    /// Only how far the lines are lit, or their routes, changed.
    Lit(String, Overlay, TextStyle),
    Hide(String),
}

#[cfg(any(target_os = "windows", test))]
impl WinOp {
    fn id(&self) -> &str {
        match self {
            Self::Show(id, _, _) | Self::Color(id, _, _) | Self::Lit(id, _, _) | Self::Hide(id) => {
                id
            }
        }
    }
}

/// Coalesce back-to-back updates for the same overlay id so high-rate color
/// or lit frames never pile up in the Win32 message loop or starve timers.
#[cfg(any(target_os = "windows", test))]
fn coalesce_win_ops(ops: Vec<WinOp>) -> Vec<WinOp> {
    let mut out: Vec<WinOp> = Vec::with_capacity(ops.len());
    for op in ops {
        let Some(pos) = out.iter().rposition(|prev| prev.id() == op.id()) else {
            out.push(op);
            continue;
        };
        match (&out[pos], op) {
            (_, WinOp::Hide(id)) => {
                out.remove(pos);
                out.push(WinOp::Hide(id));
            }
            (_, WinOp::Show(id, overlay, style)) => {
                out[pos] = WinOp::Show(id, overlay, style);
            }
            (
                WinOp::Show(..),
                WinOp::Color(id, overlay, style) | WinOp::Lit(id, overlay, style),
            ) => {
                out[pos] = WinOp::Show(id, overlay, style);
            }
            (
                WinOp::Color(..),
                WinOp::Color(id, overlay, style) | WinOp::Lit(id, overlay, style),
            ) => {
                out[pos] = WinOp::Color(id, overlay, style);
            }
            (WinOp::Lit(..), WinOp::Color(id, overlay, style)) => {
                out[pos] = WinOp::Color(id, overlay, style);
            }
            (WinOp::Lit(..), WinOp::Lit(id, overlay, style)) => {
                out[pos] = WinOp::Lit(id, overlay, style);
            }
            (WinOp::Hide(_), next) => out.push(next),
        }
    }
    out
}

/// Repaint line colors of an overlay in place without rebuilding its window or controls.
fn color_here(id: &str, overlay: &Overlay, style: TextStyle) {
    #[cfg(target_os = "macos")]
    mac::color(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "windows")]
    win::color(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_show", id, &as_json(overlay, style));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, overlay, style);
}

/// Show an overlay with this system's own windows.
fn show_here(id: &str, overlay: &Overlay, style: TextStyle) {
    #[cfg(target_os = "macos")]
    mac::show(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "windows")]
    win::show(id.to_string(), overlay.clone(), style);
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_show", id, &as_json(overlay, style));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, overlay, style);
}

fn hide_here(id: &str) {
    #[cfg(target_os = "macos")]
    mac::hide(id.to_string());
    #[cfg(target_os = "windows")]
    win::hide(id.to_string());
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_hide", id, "");
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = id;
}

/// Where the user left the overlay `id`: the middle of its bottom edge, in
/// points from the bottom left of the main screen.
pub fn saved_place(id: &str) -> Option<(f64, f64)> {
    let bytes = crate::storage::Store::host()
        .read(&format!("overlay.{id}"))
        .ok()??;
    let text = String::from_utf8(bytes).ok()?;
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// Keep where the user left the overlay `id`.
pub fn save_place(id: &str, x: f64, y: f64) {
    static KEPT: Mutex<BTreeMap<String, (f64, f64)>> = Mutex::new(BTreeMap::new());
    let was = KEPT
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_string(), (x, y));
    if was == Some((x, y)) {
        return;
    }
    let _ = crate::storage::Store::host()
        .write(&format!("overlay.{id}"), format!("{x},{y}").as_bytes());
}

/// An overlay as JSON for a host: its lines, place, lock, buttons and text style.
#[cfg(any(target_os = "android", test))]
fn as_json(overlay: &Overlay, style: TextStyle) -> String {
    fn text(value: &str) -> String {
        let mut out = String::from("\"");
        for c in value.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    let lines: Vec<String> = overlay
        .lines
        .iter()
        .map(|line| {
            let [r, g, b, a] = line.color;
            let route = line.route.as_ref().map_or_else(String::new, |route| {
                format!(
                    ",\"path\":{},\"clock\":{},\"since\":{}",
                    text(&route.path()),
                    route.clock,
                    route.since
                )
            });
            format!(
                "{{\"text\":{},\"ruby\":{},\"size\":{},\"bold\":{},\"color\":\"#{a:02x}{r:02x}{g:02x}{b:02x}\",\"lit\":{}{route}}}",
                text(&line.text),
                text(&line.ruby),
                line.size,
                line.bold,
                line.lit.unwrap_or(if line.route.is_some() { 0.0 } else { -1.0 })
            )
        })
        .collect();
    let buttons: Vec<String> = overlay
        .buttons
        .iter()
        .map(|button| {
            format!(
                "{{\"id\":{},\"symbol\":{},\"whileLocked\":{}}}",
                text(&button.id),
                text(&button.symbol),
                button.while_locked
            )
        })
        .collect();
    let effect = match style.effect {
        TextEffect::None => "none",
        TextEffect::Shadow => "shadow",
        TextEffect::Outline => "outline",
    };
    format!(
        "{{\"lines\":[{}],\"place\":\"{}\",\"locked\":{},\"buttons\":[{}],\"effect\":\"{}\",\"opacity\":{}}}",
        lines.join(","),
        match overlay.place {
            Place::Free => "free",
            Place::Taskbar => "taskbar",
        },
        overlay.locked,
        buttons.join(","),
        effect,
        style.opacity
    )
}

/// macOS: a borderless, non-activating panel above the other windows, on
/// every space and over full-screen apps, with a label per line. While the
/// pointer is over it, which a poll of the pointer tells, the bar shows
/// and, unlocked, the plate; locked, the panel takes the pointer only over
/// the bar's buttons.
#[cfg(target_os = "macos")]
mod mac {
    use super::{Line, Overlay, Route, TextEffect, TextStyle};
    use dispatch2::{DispatchQueue, DispatchTime};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject, ProtocolObject};
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSBackingStoreType, NSBox, NSBoxType, NSButton, NSColor, NSEvent, NSFont,
        NSFontAttributeName, NSFontWeightSemibold, NSForegroundColorAttributeName, NSImage,
        NSImageSymbolConfiguration, NSPanel, NSScreen, NSShadow, NSShadowAttributeName,
        NSStrokeColorAttributeName, NSStrokeWidthAttributeName, NSTextField, NSTitlePosition,
        NSUserInterfaceItemIdentification, NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_foundation::{
        NSArray, NSMutableAttributedString, NSMutableDictionary, NSNumber, NSPoint, NSRect, NSSize,
        NSString,
    };
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::time::Duration;

    /// The margin around the text, and the gap between lines.
    const MARGIN: f64 = 12.0;
    const GAP: f64 = 2.0;
    /// A button of the bar, the room between them, and between the bar and
    /// the lines.
    const BUTTON: f64 = 28.0;
    const BUTTON_GAP: f64 = 8.0;
    const BAR_GAP: f64 = 8.0;
    /// How often the pointer is looked for over a panel.
    const POLL: Duration = Duration::from_millis(80);

    #[link(name = "CoreText", kind = "framework")]
    unsafe extern "C" {
        fn CTLineCreateWithAttributedString(
            text: *const std::ffi::c_void,
        ) -> *const std::ffi::c_void;
        fn CTLineGetOffsetForStringIndex(
            line: *const std::ffi::c_void,
            index: isize,
            secondary: *mut f64,
        ) -> f64;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const std::ffi::c_void);
    }

    /// Measure an edge in the complete shaped line, including kerning and
    /// ligatures. The native field adds a two-point inset on either side.
    fn source_edge(line: &Line, byte: usize, mtm: MainThreadMarker) -> f64 {
        let field = styled(line, &line.text, TextEffect::None, 1.0, mtm);
        let text = field.attributedStringValue();
        // SAFETY: NSAttributedString is toll-free bridged to its CF type;
        // the created CTLine is released after its UTF-16 offset is read.
        unsafe {
            let shaped =
                CTLineCreateWithAttributedString((&*text as *const _) as *const std::ffi::c_void);
            if shaped.is_null() {
                return 0.0;
            }
            let edge = CTLineGetOffsetForStringIndex(
                shaped,
                line.text[..byte].encode_utf16().count() as isize,
                std::ptr::null_mut(),
            );
            CFRelease(shaped);
            edge
        }
    }

    /// A panel and what changes as the pointer comes and goes.
    struct Shown {
        id: String,
        panel: Retained<NSPanel>,
        plate: Option<Retained<NSBox>>,
        /// Each button's round, and the button.
        buttons: Vec<(Retained<NSBox>, Retained<NSButton>)>,
        locked: bool,
        allows_sync: bool,
        last_frame: Cell<NSRect>,
        /// What is lit of each line that shows, for those that light up.
        lit: Vec<Option<Lit>>,
        /// The text fields painted for each visible line, updated in place on color changes.
        fields: Vec<Vec<PaintField>>,
    }

    struct PaintField {
        field: Retained<NSTextField>,
        text: String,
        size: f64,
        bold: bool,
        alpha: f64,
    }

    /// What is lit of a line: the view that cuts it off.
    struct Lit {
        line: Line,
        clip: Retained<NSView>,
        size: NSSize,
        /// The width of the beginnings of the line measured so far, by
        /// where they end.
        widths: HashMap<usize, f64>,
    }

    impl Lit {
        fn width_to(&mut self, at: usize, mtm: MainThreadMarker) -> f64 {
            let line = &self.line;
            *self.widths.entry(at).or_insert_with(|| {
                if line.ruby_spans().is_empty() {
                    styled(line, &line.text[..at], TextEffect::None, 1.0, mtm)
                        .frame()
                        .size
                        .width
                } else {
                    source_edge(line, at, mtm) + 4.0
                }
            })
        }

        /// How wide the line is as far as `place`.
        fn width_at(&mut self, place: f64, mtm: MainThreadMarker) -> f64 {
            let text = &self.line.text;
            let Some((at, now)) = text.char_indices().nth(place as usize) else {
                return self.size.width;
            };
            let next = at + now.len_utf8();
            let sung = self.width_to(at, mtm);
            // A label keeps two points on either side of its text.
            sung - 2.0 + (self.width_to(next, mtm) - sung) * place.fract()
        }

        /// Cut the line off where it is lit now. On a route the cut goes on
        /// by itself: Core Animation moves it, off this thread.
        fn show(&mut self, mtm: MainThreadMarker) {
            // SAFETY: a view answers `layer`, here with its backing layer.
            let layer: Option<Retained<AnyObject>> = unsafe { msg_send![&*self.clip, layer] };
            let key = NSString::from_str("lit");
            if let Some(layer) = &layer {
                // SAFETY: a layer, and a key of its animations.
                let _: () = unsafe { msg_send![&**layer, removeAnimationForKey: &*key] };
            }
            let width = match self.line.lit_now() {
                Some(place) => self.width_at(place, mtm),
                None => self.size.width,
            };
            self.clip.setFrameSize(NSSize::new(width, self.size.height));
            let Some(route) = self.line.route.clone().filter(Route::moving) else {
                return;
            };
            let begin = route.second();
            let steps = route.steps(begin);
            let Some(length) = steps
                .last()
                .map(|(second, _)| second - begin)
                .filter(|length| *length > 0.0)
            else {
                return;
            };
            let class = AnyClass::get(c"CAKeyframeAnimation");
            let (Some(class), Some(layer)) = (class, layer) else {
                return;
            };
            let mut widths = Vec::with_capacity(steps.len());
            let mut times = Vec::with_capacity(steps.len());
            for (second, place) in steps {
                widths.push(NSNumber::new_f64(self.width_at(place, mtm)));
                times.push(NSNumber::new_f64((second - begin) / length));
            }
            if let Some(last) = widths.last() {
                self.clip
                    .setFrameSize(NSSize::new(last.as_f64(), self.size.height));
            }
            let path = NSString::from_str("bounds.size.width");
            // SAFETY: Core Animation's class, its documented properties with
            // values of their kinds, and a layer that copies the animation.
            unsafe {
                let motion: Retained<AnyObject> = msg_send![class, animationWithKeyPath: &*path];
                let _: () = msg_send![&*motion, setValues: &*NSArray::from_retained_slice(&widths)];
                let _: () =
                    msg_send![&*motion, setKeyTimes: &*NSArray::from_retained_slice(&times)];
                let _: () = msg_send![&*motion, setDuration: length];
                let _: () = msg_send![&*layer, addAnimation: &*motion, forKey: &*key];
            }
        }
    }

    thread_local! {
        static PANELS: RefCell<HashMap<String, Shown>> = RefCell::new(HashMap::new());
        static POLLING: Cell<bool> = const { Cell::new(false) };
        static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and `Target`
        // does not implement `Drop`.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "BernOverlayTarget"]
        struct Target;

        impl Target {
            /// A button's identifier is its overlay's id and its own,
            /// apart by a unit separator.
            #[unsafe(method(press:))]
            fn press(&self, sender: &NSButton) {
                if let Some(name) = sender.identifier() {
                    let name = name.to_string();
                    if let Some((overlay, button)) = name.split_once('\u{1f}') {
                        super::press(overlay, button);
                    }
                }
            }
        }
    );

    fn target(mtm: MainThreadMarker) -> Retained<Target> {
        TARGET.with(|target| {
            target
                .borrow_mut()
                .get_or_insert_with(|| {
                    let this = Target::alloc(mtm).set_ivars(());
                    // SAFETY: NSObject's own initializer.
                    unsafe { msg_send![super(this), init] }
                })
                .clone()
        })
    }

    pub fn show(id: String, overlay: Overlay, style: TextStyle) {
        DispatchQueue::main().exec_async(move || {
            if let Some(mtm) = MainThreadMarker::new() {
                put(mtm, &id, &overlay, style);
            }
        });
    }

    /// Only how far the lines are lit changed: the panel stays as it is.
    pub fn lit(id: String, overlay: Overlay, style: TextStyle) {
        DispatchQueue::main().exec_async(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let fallback = PANELS.with(|panels| {
                let mut panels = panels.borrow_mut();
                let Some(shown) = panels.get_mut(&id) else {
                    return Some(style);
                };
                let lines = overlay.lines.iter().filter(|line| !line.text.is_empty());
                for (lit, line) in shown.lit.iter_mut().zip(lines) {
                    if let Some(lit) = lit {
                        lit.line.lit = line.lit;
                        lit.line.route = line.route.clone();
                        lit.show(mtm);
                    }
                }
                None
            });
            if let Some(style) = fallback {
                put(mtm, &id, &overlay, style);
            }
        });
    }

    /// Only line colors (and optionally lit timing) changed: repaint existing text fields in place.
    pub fn color(id: String, overlay: Overlay, style: TextStyle) {
        DispatchQueue::main().exec_async(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let fallback = PANELS.with(|panels| {
                let mut panels = panels.borrow_mut();
                let Some(shown) = panels.get_mut(&id) else {
                    return Some(style);
                };
                let lines: Vec<&Line> = overlay
                    .lines
                    .iter()
                    .filter(|line| !line.text.is_empty())
                    .collect();
                if shown.fields.len() != lines.len() || shown.lit.len() != lines.len() {
                    return Some(style);
                }
                for ((fields, lit_slot), line) in
                    shown.fields.iter().zip(shown.lit.iter_mut()).zip(lines)
                {
                    for paint in fields {
                        let mut styled_line = line.clone();
                        styled_line.size = paint.size;
                        styled_line.bold = paint.bold;
                        let text = attributed(&styled_line, &paint.text, style.effect, paint.alpha);
                        paint.field.setAttributedStringValue(&text);
                    }
                    if let Some(lit) = lit_slot {
                        let timing_changed =
                            lit.line.lit != line.lit || lit.line.route != line.route;
                        lit.line.color = line.color;
                        if timing_changed {
                            lit.line.lit = line.lit;
                            lit.line.route = line.route.clone();
                            lit.show(mtm);
                        }
                    }
                }
                None
            });
            if let Some(style) = fallback {
                put(mtm, &id, &overlay, style);
            }
        });
    }

    pub fn hide(id: String) {
        DispatchQueue::main().exec_async(move || {
            let parent = PANELS.with(|panels| panels.borrow().get(&id).map(|s| s.panel.clone()));
            if let Some(parent) = parent {
                keep_place(&id, &parent);
                parent.orderOut(None);
            }
            super::update_rect(&id, None);
        });
    }

    /// Remember where the panel is: the middle of its bottom edge.
    fn keep_place(id: &str, panel: &NSPanel) {
        let frame = panel.frame();
        super::save_place(id, frame.origin.x + frame.size.width / 2.0, frame.origin.y);
    }

    /// A line: faded, and over it what is lit, cut off inside the character
    /// `Line::lit` stops in.
    fn label(
        line: &Line,
        style: TextStyle,
        mtm: MainThreadMarker,
    ) -> (Retained<NSView>, Option<Lit>, Vec<PaintField>) {
        let lights = line.lights();
        let (base, mut fields) = annotated(
            line,
            style.effect,
            if lights { super::FADED } else { 1.0 },
            mtm,
        );
        let size = base.frame().size;
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), size);
        let label = NSView::initWithFrame(NSView::alloc(mtm), frame);
        label.setWantsLayer(true);
        label.setAlphaValue(style.opacity);
        label.addSubview(&base);
        if !lights {
            return (label, None, fields);
        }
        let clip = NSView::initWithFrame(NSView::alloc(mtm), frame);
        clip.setClipsToBounds(true);
        // The text, with its blurred shadow, is drawn once: moving the cut
        // only changes the bounds of a layer.
        clip.setWantsLayer(true);
        let (bright, bright_fields) = annotated(line, style.effect, 1.0, mtm);
        fields.extend(bright_fields);
        bright.setWantsLayer(true);
        // SAFETY: `NSViewLayerContentsRedrawNever` is 0.
        let _: () = unsafe { msg_send![&*clip, setLayerContentsRedrawPolicy: 0isize] };
        clip.addSubview(&bright);
        label.addSubview(&clip);
        let mut lit = Lit {
            line: line.clone(),
            clip,
            size,
            widths: HashMap::new(),
        };
        lit.show(mtm);
        (label, Some(lit), fields)
    }

    /// Keep the original line as one shaped, accessible text field. Readings
    /// occupy a reserved band above their measured source ranges; they do not
    /// contribute characters to the highlighting clock.
    fn annotated(
        line: &Line,
        effect: TextEffect,
        alpha: f64,
        mtm: MainThreadMarker,
    ) -> (Retained<NSView>, Vec<PaintField>) {
        let base = styled(line, &line.text, effect, alpha, mtm);
        let mut fields = vec![PaintField {
            field: base.clone(),
            text: line.text.clone(),
            size: line.size,
            bold: line.bold,
            alpha,
        }];
        let spans = line.ruby_spans();
        if spans.is_empty() {
            return (base.into_super().into_super(), fields);
        }
        let size = base.frame().size;
        let band = line.size * 0.7 + 4.0;
        let view = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(size.width, size.height + band),
            ),
        );
        view.addSubview(&base);
        let offsets: Vec<_> = line
            .text
            .char_indices()
            .map(|(at, _)| at)
            .chain(std::iter::once(line.text.len()))
            .collect();
        for (start, end, reading) in spans {
            let start_edge = source_edge(line, offsets[start], mtm) + 2.0;
            let end_edge = source_edge(line, offsets[end], mtm) + 2.0;
            let left = start_edge.min(end_edge);
            let width = (end_edge - start_edge).abs().max(1.0);
            let mut style = line.clone();
            style.size *= 0.45;
            let measured = styled(&style, &reading, TextEffect::None, 1.0, mtm)
                .frame()
                .size
                .width;
            style.size *= (width / measured.max(1.0)).min(1.0);
            let ruby = styled(&style, &reading, effect, alpha, mtm);
            let ruby_size = ruby.frame().size;
            ruby.setFrameOrigin(NSPoint::new(
                left + (width - ruby_size.width) / 2.0,
                size.height,
            ));
            // The original field remains the accessible text, without a
            // second traversal through the pronunciation decorations.
            let _: () = unsafe { msg_send![&*ruby, setAccessibilityElement: false] };
            view.addSubview(&ruby);
            fields.push(PaintField {
                field: ruby,
                text: reading,
                size: style.size,
                bold: style.bold,
                alpha,
            });
        }
        (view, fields)
    }

    fn attributed(
        line: &Line,
        text: &str,
        effect: TextEffect,
        alpha: f64,
    ) -> Retained<NSMutableAttributedString> {
        let attributes = NSMutableDictionary::<NSString, AnyObject>::new();
        let font = if line.bold {
            NSFont::boldSystemFontOfSize(line.size)
        } else {
            NSFont::systemFontOfSize(line.size)
        };
        let [r, g, b, a] = line.color.map(|c| c as f64 / 255.0);
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a * alpha);
        // SAFETY: the keys are AppKit's attribute names, with values of their
        // kinds.
        unsafe {
            attributes.setObject_forKey(&font, ProtocolObject::from_ref(NSFontAttributeName));
            attributes.setObject_forKey(
                &color,
                ProtocolObject::from_ref(NSForegroundColorAttributeName),
            );
            match effect {
                TextEffect::None => {}
                TextEffect::Shadow => {
                    let shadow = NSShadow::new();
                    shadow.setShadowOffset(NSSize::new(0.0, -1.0));
                    shadow.setShadowBlurRadius(3.0);
                    shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0.0, 0.85)));
                    attributes
                        .setObject_forKey(&shadow, ProtocolObject::from_ref(NSShadowAttributeName));
                }
                TextEffect::Outline => {
                    let stroke = NSColor::colorWithWhite_alpha(0.0, 0.9);
                    let width = NSNumber::new_f64(-3.5);
                    attributes.setObject_forKey(
                        &stroke,
                        ProtocolObject::from_ref(NSStrokeColorAttributeName),
                    );
                    attributes.setObject_forKey(
                        &width,
                        ProtocolObject::from_ref(NSStrokeWidthAttributeName),
                    );
                }
            }
        }
        // SAFETY: string attributes as AppKit takes them.
        unsafe {
            NSMutableAttributedString::initWithString_attributes(
                NSMutableAttributedString::alloc(),
                &NSString::from_str(text),
                Some(&attributes),
            )
        }
    }

    /// `text` in the style of `line`, its alpha scaled by `alpha`.
    fn styled(
        line: &Line,
        text: &str,
        effect: TextEffect,
        alpha: f64,
        mtm: MainThreadMarker,
    ) -> Retained<NSTextField> {
        let text = attributed(line, text, effect, alpha);
        let label = NSTextField::labelWithAttributedString(&text, mtm);
        label.sizeToFit();
        label
    }

    /// A rounded, filled box without a border or title.
    fn round(frame: NSRect, radius: f64, mtm: MainThreadMarker) -> Retained<NSBox> {
        let round = NSBox::initWithFrame(NSBox::alloc(mtm), frame);
        round.setBoxType(NSBoxType::Custom);
        round.setTitlePosition(NSTitlePosition::NoTitle);
        round.setBorderWidth(0.0);
        round.setCornerRadius(radius);
        round.setContentViewMargins(NSSize::new(0.0, 0.0));
        round.setFillColor(&NSColor::colorWithWhite_alpha(0.0, 0.3));
        round
    }

    fn button(
        overlay: &str,
        button: &super::Button,
        frame: NSRect,
        mtm: MainThreadMarker,
    ) -> Retained<NSButton> {
        let symbol = NSString::from_str(&button.symbol);
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol, None)
            .unwrap_or_default();
        // SAFETY: NSFontWeightSemibold is AppKit's constant.
        let weight = unsafe { NSFontWeightSemibold };
        let configuration =
            NSImageSymbolConfiguration::configurationWithPointSize_weight(13.0, weight);
        let image = image
            .imageWithSymbolConfiguration(&configuration)
            .unwrap_or(image);
        let target = target(mtm);
        // SAFETY: the target is kept in `TARGET` for as long as the
        // thread lives, and answers `press:`.
        let view = unsafe {
            NSButton::buttonWithImage_target_action(&image, Some(&target), Some(sel!(press:)), mtm)
        };
        view.setBordered(false);
        view.setContentTintColor(Some(&NSColor::whiteColor()));
        view.setFrame(frame);
        view.setIdentifier(Some(&NSString::from_str(&format!(
            "{overlay}\u{1f}{}",
            button.id
        ))));
        view
    }

    fn put(mtm: MainThreadMarker, id: &str, overlay: &Overlay, style: TextStyle) {
        let panel = PANELS.with(|panels| {
            let mut panels = panels.borrow_mut();
            if let Some(shown) = panels.get_mut(id) {
                shown.panel.clone()
            } else {
                let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(200.0, 60.0));
                let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
                    NSPanel::alloc(mtm),
                    frame,
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                    NSBackingStoreType::Buffered,
                    false,
                );
                panel.setOpaque(false);
                panel.setHasShadow(false);
                panel.setBackgroundColor(Some(&NSColor::clearColor()));
                // A floating panel takes the floating level; the status level
                // after it keeps it above other apps' panels too.
                panel.setFloatingPanel(true);
                panel.setLevel(objc2_app_kit::NSStatusWindowLevel);
                panel.setCollectionBehavior(
                    NSWindowCollectionBehavior::CanJoinAllSpaces
                        | NSWindowCollectionBehavior::Stationary
                        | NSWindowCollectionBehavior::FullScreenAuxiliary
                        | NSWindowCollectionBehavior::IgnoresCycle,
                );
                panel.setMovableByWindowBackground(true);
                panel.setHidesOnDeactivate(false);
                panel.setBecomesKeyOnlyIfNeeded(true);
                // It is put back from the place kept for it; left restorable,
                // each change has AppKit encode the whole app's windows.
                // SAFETY: `setRestorable:` takes a BOOL.
                let _: () = unsafe { msg_send![&*panel, setRestorable: false] };
                // SAFETY: the panel stays in `PANELS` and is never closed.
                unsafe { panel.setReleasedWhenClosed(false) };
                panel
            }
        });
        let was_visible = panel.isVisible();
        // Where it was: the middle of its bottom edge, kept as the text
        // changes width.
        let place = if was_visible {
            let frame = panel.frame();
            Some((frame.origin.x + frame.size.width / 2.0, frame.origin.y))
        } else {
            super::saved_place(id)
        };
        let mut labels = Vec::new();
        let mut lit = Vec::new();
        let mut fields = Vec::new();
        for line in overlay.lines.iter().filter(|line| !line.text.is_empty()) {
            let (view, line_lit, line_fields) = label(line, style, mtm);
            labels.push(view);
            lit.push(line_lit);
            fields.push(line_fields);
        }
        // None on the taskbar. Locked, only the buttons that unlock it, each
        // where it is when unlocked.
        let buttons: Vec<&super::Button> = if overlay.place == super::Place::Free {
            overlay.buttons.iter().collect()
        } else {
            Vec::new()
        };
        let bar_width =
            buttons.len() as f64 * BUTTON + buttons.len().saturating_sub(1) as f64 * BUTTON_GAP;
        let bar_height = if buttons.is_empty() {
            0.0
        } else {
            BUTTON + BAR_GAP
        };
        let width = labels
            .iter()
            .map(|label| label.frame().size.width)
            .fold(bar_width, f64::max)
            + 2.0 * MARGIN;
        let height = labels
            .iter()
            .map(|label| label.frame().size.height)
            .sum::<f64>()
            + GAP * labels.len().saturating_sub(1) as f64
            + bar_height
            + 2.0 * MARGIN;
        let content = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
        );
        let plate = (!overlay.locked).then(|| {
            let plate = round(
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
                12.0,
                mtm,
            );
            content.addSubview(&plate);
            plate
        });
        let mut top = height - MARGIN;
        let mut left = (width - bar_width) / 2.0;
        let buttons: Vec<(Retained<NSBox>, Retained<NSButton>)> = buttons
            .into_iter()
            .filter_map(|description| {
                let frame = NSRect::new(
                    NSPoint::new(left, top - BUTTON),
                    NSSize::new(BUTTON, BUTTON),
                );
                left += BUTTON + BUTTON_GAP;
                if overlay.locked && !description.while_locked {
                    return None;
                }
                let back = round(frame, BUTTON / 2.0, mtm);
                let button = button(id, description, frame, mtm);
                content.addSubview(&back);
                content.addSubview(&button);
                Some((back, button))
            })
            .collect();
        top -= bar_height;
        for label in &labels {
            let size = label.frame().size;
            top -= size.height;
            label.setFrameOrigin(NSPoint::new((width - size.width) / 2.0, top));
            top -= GAP;
            content.addSubview(label);
        }
        panel.setContentView(Some(&content));
        let (x, y) = place.unwrap_or_else(|| {
            let screen = NSScreen::mainScreen(mtm)
                .map(|screen| screen.visibleFrame())
                .unwrap_or(NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(1440.0, 900.0),
                ));
            (
                screen.origin.x + screen.size.width / 2.0,
                screen.origin.y + screen.size.height * 0.1,
            )
        });
        panel.setFrame_display(
            NSRect::new(NSPoint::new(x - width / 2.0, y), NSSize::new(width, height)),
            true,
        );
        let empty = overlay.lines.iter().all(|line| line.text.is_empty());
        let allows_sync = !empty && !overlay.locked && overlay.place == super::Place::Free;
        let frame = panel.frame();
        let shown = Shown {
            id: id.to_string(),
            panel: panel.clone(),
            plate,
            buttons,
            locked: overlay.locked,
            allows_sync,
            last_frame: Cell::new(frame),
            lit,
            fields,
        };
        hover(&shown);
        PANELS.with(|panels| panels.borrow_mut().insert(id.to_string(), shown));
        if empty {
            panel.orderOut(None);
            super::update_rect(id, None);
        } else {
            if !was_visible {
                panel.orderFrontRegardless();
            }
            if allows_sync {
                super::update_rect(
                    id,
                    Some((
                        frame.origin.x,
                        frame.origin.y,
                        frame.size.width,
                        frame.size.height,
                    )),
                );
            } else {
                super::update_rect(id, None);
            }
            poll();
        }
        keep_place(id, &panel);
    }

    /// Show the bar and the plate while the pointer is over the panel;
    /// locked, take the pointer only over a button.
    fn hover(shown: &Shown) {
        let pointer = NSEvent::mouseLocation();
        let frame = shown.panel.frame();
        if shown.panel.isVisible() && frame != shown.last_frame.get() {
            shown.last_frame.set(frame);
            keep_place(&shown.id, &shown.panel);
            if shown.allows_sync {
                super::update_rect(
                    &shown.id,
                    Some((
                        frame.origin.x,
                        frame.origin.y,
                        frame.size.width,
                        frame.size.height,
                    )),
                );
            }
        }
        let inside = |rect: NSRect| {
            pointer.x >= rect.origin.x
                && pointer.x < rect.origin.x + rect.size.width
                && pointer.y >= rect.origin.y
                && pointer.y < rect.origin.y + rect.size.height
        };
        let over = shown.panel.isVisible() && inside(frame);
        if let Some(plate) = &shown.plate {
            plate.setHidden(!over);
        }
        let mut on_button = false;
        for (back, button) in &shown.buttons {
            let rect = button.frame();
            let on = over
                && inside(NSRect::new(
                    NSPoint::new(
                        frame.origin.x + rect.origin.x,
                        frame.origin.y + rect.origin.y,
                    ),
                    rect.size,
                ));
            on_button |= on;
            back.setHidden(!over);
            button.setHidden(!over);
            back.setFillColor(&NSColor::colorWithWhite_alpha(
                0.0,
                if on { 0.55 } else { 0.3 },
            ));
        }
        shown
            .panel
            .setIgnoresMouseEvents(shown.locked && !on_button);
    }

    /// Look for the pointer over the panels while any shows.
    fn poll() {
        if POLLING.with(|polling| polling.replace(true)) {
            return;
        }
        fn again() {
            let any = PANELS.with(|panels| {
                let panels = panels.borrow();
                panels.values().for_each(hover);
                panels.values().any(|shown| shown.panel.isVisible())
            });
            if !any {
                POLLING.with(|polling| polling.set(false));
                return;
            }
            if let Ok(when) = DispatchTime::try_from(POLL) {
                let _ = DispatchQueue::main().after(when, again);
            }
        }
        again();
    }
}

/// Windows: a layered, top-most tool window per overlay, drawn with
/// Direct2D and DirectWrite into a bitmap whose alpha the window takes, on
/// a thread of its own with its message loop. A timer looks for the pointer
/// over a free overlay: there the bar shows and, unlocked, the plate.
/// Unlocked, the window drags but by its buttons; locked, it lets clicks
/// through but over a button that shows.
/// Free stretch of the Windows taskbar an overlay stands in, in screen pixels.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Room {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl Room {
    fn width(self) -> i32 {
        (self.right - self.left).max(0)
    }
}

/// Between an overlay on the taskbar and nearby taskbar controls, in pixels.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const TASKBAR_GAP: i32 = 8;

/// Extra margin given to the currently chosen taskbar gap so small icon shifts
/// do not make the overlay bounce between the left and right sides.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const TASKBAR_STICKY: i32 = 48;

/// Clamp, sort and coalesce horizontal taskbar occupancy intervals `[left, right)`
/// within `[bar_left, bar_right)`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn merge_intervals(raw: &[(i32, i32)], bar_left: i32, bar_right: i32) -> Vec<(i32, i32)> {
    let mut spans: Vec<(i32, i32)> = raw
        .iter()
        .filter_map(|&(l, r)| {
            let l = l.max(bar_left);
            let r = r.min(bar_right);
            (r > l).then_some((l, r))
        })
        .collect();
    spans.sort_unstable_by_key(|&(l, r)| (l, r));
    let mut merged: Vec<(i32, i32)> = Vec::with_capacity(spans.len());
    for (l, r) in spans {
        if let Some(last) = merged.last_mut()
            && l <= last.1 + 2
        {
            last.1 = last.1.max(r);
        } else {
            merged.push((l, r));
        }
    }
    merged
}

/// Choose the usable horizontal room on the taskbar from all occupied control
/// spans (start, search, task buttons, widgets, system tray), avoiding controls
/// on both the left and right sides of the bar.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn pick_taskbar_room(
    bar: Room,
    occupied: &[(i32, i32)],
    gap: i32,
    min_width: i32,
    want_width: i32,
    current: Option<Room>,
) -> Option<Room> {
    if bar.bottom - bar.top > bar.right - bar.left {
        let room = Room {
            left: bar.left + gap,
            right: bar.right - gap,
            top: bar.top,
            bottom: bar.bottom,
        };
        return (room.width() >= min_width).then_some(room);
    }

    let merged = merge_intervals(occupied, bar.left, bar.right);
    let mut gaps = Vec::new();
    let mut edge = bar.left;
    for (l, r) in merged {
        if l > edge {
            let usable = (edge + gap, l - gap);
            if usable.1 - usable.0 >= min_width {
                gaps.push(usable);
            }
        }
        edge = edge.max(r);
    }
    if bar.right > edge {
        let usable = (edge + gap, bar.right - gap);
        if usable.1 - usable.0 >= min_width {
            gaps.push(usable);
        }
    }
    if gaps.is_empty() {
        return None;
    }

    let widest = *gaps
        .iter()
        .max_by_key(|&&(l, r)| (r - l, r))
        .expect("non-empty gaps");

    if let Some(prev) = current
        && let Some(&cur) = gaps
            .iter()
            .filter(|&&(l, r)| r.min(prev.right) > l.max(prev.left))
            .max_by_key(|&&(l, r)| r.min(prev.right) - l.max(prev.left))
    {
        let cur_w = cur.1 - cur.0;
        let widest_w = widest.1 - widest.0;
        if cur_w >= want_width || cur_w + TASKBAR_STICKY >= widest_w {
            return Some(Room {
                left: cur.0,
                right: cur.1,
                top: bar.top,
                bottom: bar.bottom,
            });
        }
    }

    let mid = (bar.left + bar.right) / 2;
    let chosen = gaps
        .iter()
        .copied()
        .filter(|&(l, r)| r > mid && r - l >= want_width)
        .max_by_key(|&(_, r)| r)
        .or_else(|| {
            gaps.iter()
                .copied()
                .filter(|&(l, r)| r - l >= want_width)
                .max_by_key(|&(l, r)| (r - l, r))
        })
        .unwrap_or(widest);

    Some(Room {
        left: chosen.0,
        right: chosen.1,
        top: bar.top,
        bottom: bar.bottom,
    })
}

#[cfg(target_os = "windows")]
mod win {
    use super::{
        Line, Overlay, Place, Room, Route, TASKBAR_GAP, TextEffect, TextStyle, merge_intervals,
        pick_taskbar_room,
    };
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Condvar, Mutex, OnceLock, mpsc};
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
    use windows::Win32::Graphics::Direct2D::Common::{
        D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
    };
    use windows::Win32::Graphics::Direct2D::{
        D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
        D2D1_RENDER_TARGET_USAGE_NONE, D2D1_ROUNDED_RECT, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
        D2D1CreateFactory, ID2D1Factory,
    };
    use windows::Win32::Graphics::DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_HIT_TEST_METRICS,
        DWRITE_TEXT_METRICS, DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
        DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
        IDWriteTextLayout,
    };
    use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
    use windows::Win32::Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush,
        GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, ScreenToClient, SelectObject,
    };
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationCondition, IUIAutomationElement,
        TreeScope_Children, UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID, UIA_CheckBoxControlTypeId,
        UIA_ComboBoxControlTypeId, UIA_CustomControlTypeId, UIA_EditControlTypeId,
        UIA_GroupControlTypeId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
        UIA_MenuItemControlTypeId, UIA_PaneControlTypeId, UIA_RadioButtonControlTypeId,
        UIA_SplitButtonControlTypeId, UIA_ToolBarControlTypeId, UIA_WindowControlTypeId,
    };
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetThreadDpiAwarenessContext,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, FindWindowExW,
        FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetCursorPos, GetMessageW, GetParent,
        GetWindowLongPtrW, GetWindowRect, HTCAPTION, HTCLIENT, HTTRANSPARENT, HWND_MESSAGE,
        HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IDC_ARROW, IsWindow, IsWindowVisible, KillTimer,
        LoadCursorW, MA_NOACTIVATE, MSG, PostMessageW, PostThreadMessageW, RegisterClassExW,
        SET_WINDOW_POS_FLAGS, SPI_GETWORKAREA, SW_HIDE, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SetParent, SetTimer, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, SystemParametersInfoW, TranslateMessage, ULW_ALPHA,
        UPDATELAYEREDWINDOWINFO, UpdateLayeredWindow, UpdateLayeredWindowIndirect, WINDOW_EX_STYLE,
        WINDOW_STYLE, WM_APP, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP,
        WM_MOUSEACTIVATE, WM_NCHITTEST, WM_TIMER, WNDCLASSEXW, WS_CHILD, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };
    use windows::core::{BOOL, PCWSTR, w};
    use windows_numerics::Vector2;

    use super::{WinOp as Message, coalesce_win_ops};

    /// `(sender, wake_hwnd)` where `wake_hwnd` is a message-only window so
    /// `WM_APP` is dispatched even while modal Win32 dialogs (`ChooseColorW`)
    /// run their own message loops.
    static CHANNEL: Mutex<Option<(mpsc::Sender<Message>, isize)>> = Mutex::new(None);

    /// Posted to the overlay thread when the background taskbar scan updates.
    const WM_TASKBAR_ROOM: u32 = WM_APP + 1;

    /// Around the text and between lines, in points.
    const MARGIN: f32 = 12.0;
    const GAP: f32 = 2.0;
    /// A button of the bar, the room between them, and between the bar and
    /// the lines, in points.
    const BUTTON: f32 = 28.0;
    const BUTTON_GAP: f32 = 8.0;
    const BAR_GAP: f32 = 8.0;
    /// How often a taskbar overlay keeps its child z-order and checks whether
    /// the taskbar window itself was recreated.
    const TASKBAR_TIMER: usize = 1;
    /// Taskbar UIA scan cadence: fast right after changes, backing off while idle
    /// to bound native UIA allocations.
    const SCAN_FAST: Duration = Duration::from_millis(1000);
    const SCAN_MAX: Duration = Duration::from_secs(30);
    /// Leaf control types that occupy taskbar space directly without recursing.
    const LEAF_OCCUPYING: [UIA_CONTROLTYPE_ID; 8] = [
        UIA_ButtonControlTypeId,
        UIA_SplitButtonControlTypeId,
        UIA_MenuItemControlTypeId,
        UIA_ListItemControlTypeId,
        UIA_EditControlTypeId,
        UIA_ComboBoxControlTypeId,
        UIA_CheckBoxControlTypeId,
        UIA_RadioButtonControlTypeId,
    ];
    /// Pure container control types ignored when empty so layout spacers do not
    /// block taskbar room.
    const CONTAINER_TYPES: [UIA_CONTROLTYPE_ID; 6] = [
        UIA_PaneControlTypeId,
        UIA_WindowControlTypeId,
        UIA_GroupControlTypeId,
        UIA_ListControlTypeId,
        UIA_ToolBarControlTypeId,
        UIA_CustomControlTypeId,
    ];
    /// How often the pointer is looked for over a free overlay, in
    /// milliseconds.
    const HOVER_TIMER: usize = 2;
    const HOVER_EVERY: u32 = 80;
    /// How often a line on a route moves on, in milliseconds.
    const LIT_TIMER: usize = 3;
    const LIT_EVERY: u32 = 16;

    /// The icon font's glyphs for the symbols the bar draws.
    const GLYPHS: &[(&str, u16)] = &[
        ("backward.fill", 0xEB9E),
        ("forward.fill", 0xEB9D),
        ("gearshape", 0xE713),
        ("gearshape.fill", 0xE713),
        ("lock.fill", 0xE72E),
        ("lock.open.fill", 0xE785),
        ("pause.fill", 0xF8AE),
        ("play.fill", 0xF5B0),
        ("xmark", 0xE711),
    ];

    pub fn show(id: String, overlay: Overlay, style: TextStyle) {
        send(Message::Show(id, overlay, style));
    }

    pub fn color(id: String, overlay: Overlay, style: TextStyle) {
        send(Message::Color(id, overlay, style));
    }

    pub fn lit(id: String, overlay: Overlay, style: TextStyle) {
        send(Message::Lit(id, overlay, style));
    }

    pub fn hide(id: String) {
        send(Message::Hide(id));
    }

    fn send(message: Message) {
        let mut channel = CHANNEL.lock().unwrap_or_else(|e| e.into_inner());
        if channel.is_none() {
            let (send, receive) = mpsc::channel();
            let (ready, started) = mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("bern-overlay".into())
                .spawn(move || run(receive, ready));
            let Ok(_) = spawned else { return };
            let Ok(wake_hwnd) = started.recv() else {
                return;
            };
            *channel = Some((send, wake_hwnd));
        }
        if let Some((sender, wake_hwnd)) = channel.as_ref()
            && sender.send(message).is_ok()
        {
            // SAFETY: posts to this thread's message-only wake window.
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(*wake_hwnd as *mut _)),
                    WM_APP,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
    }

    /// An overlay's window and where it stands: the middle of its bottom
    /// edge, in pixels.
    struct Window {
        id: String,
        hwnd: HWND,
        overlay: Overlay,
        place: Option<(f64, f64)>,
        /// The pointer is over the window, and over this button.
        over: bool,
        hot: Option<usize>,
        /// The button the left button went down on.
        pressed: Option<usize>,
        /// The buttons that show, each where it is in the window in pixels.
        buttons: Vec<(RECT, usize)>,
        /// What it shows now.
        frame: Option<Frame>,
        /// On the taskbar, the `Shell_TrayWnd` it is attached to as a child.
        parent: Option<HWND>,
        /// Whether this overlay is currently requested to be shown.
        visible: bool,
        /// On the taskbar, the room it had when it was last drawn.
        room: Option<Room>,
        style: TextStyle,
        /// Whether `HOVER_TIMER` is currently armed on `hwnd`.
        hover_timer: bool,
        /// Whether `TASKBAR_TIMER` is currently armed on `hwnd`.
        taskbar_timer: bool,
        /// Whether `LIT_TIMER` is currently armed on `hwnd`.
        lit_timer: bool,
        /// True while the user is dragging the window inside Win32's move loop.
        dragging: bool,
    }

    #[derive(Clone, PartialEq, Eq)]
    struct TaskbarSnapshot {
        tray: isize,
        bar: Room,
        occupied: Vec<(i32, i32)>,
    }

    struct ScannerState {
        overlay_thread: u32,
        target: Option<(isize, isize)>,
        snapshot: Option<TaskbarSnapshot>,
        nudge: bool,
    }

    struct Scanner {
        state: Mutex<ScannerState>,
        wake: Condvar,
    }

    static SCANNER: OnceLock<Scanner> = OnceLock::new();

    fn scanner() -> &'static Scanner {
        SCANNER.get_or_init(|| {
            let s = Scanner {
                state: Mutex::new(ScannerState {
                    overlay_thread: 0,
                    target: None,
                    snapshot: None,
                    nudge: false,
                }),
                wake: Condvar::new(),
            };
            let _ = std::thread::Builder::new()
                .name("bern-taskbar-scan".into())
                .spawn(scan_loop);
            s
        })
    }

    fn set_scanner_thread(thread: u32) {
        let s = scanner();
        let mut guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.overlay_thread = thread;
    }

    fn refresh_scanner(windows: &HashMap<String, Window>) {
        let target = windows
            .values()
            .find(|w| w.visible && w.overlay.place == Place::Taskbar)
            .and_then(|w| {
                let tray = find_taskbar()?;
                Some((tray.0 as isize, w.hwnd.0 as isize))
            });
        let s = scanner();
        let mut guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.target != target {
            guard.target = target;
            if target.is_none() {
                guard.snapshot = None;
            } else {
                guard.nudge = true;
            }
            s.wake.notify_one();
        }
    }

    fn latest_taskbar_snapshot(tray: HWND) -> Option<TaskbarSnapshot> {
        let s = SCANNER.get()?;
        let guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .snapshot
            .as_ref()
            .filter(|snap| snap.tray == tray.0 as isize)
            .cloned()
    }

    fn scan_loop() {
        // SAFETY: background thread sets its own DPI awareness and MTA COM
        // apartment once so UIA rects match physical screen pixels without
        // blocking the overlay message loop.
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let automation: Option<IUIAutomation> =
            // SAFETY: COM is initialized on this thread.
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();

        let s = scanner();
        let mut prev_key: Option<(isize, isize, Room)> = None;
        let mut prev_raw: Vec<(i32, i32)> = Vec::new();
        let mut settled_streak = 0u32;
        let mut idle = SCAN_FAST;

        loop {
            let target = {
                let mut guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
                while guard.target.is_none() {
                    prev_key = None;
                    prev_raw.clear();
                    settled_streak = 0;
                    idle = SCAN_FAST;
                    guard = s.wake.wait(guard).unwrap_or_else(|e| e.into_inner());
                }
                guard.nudge = false;
                guard.target
            };
            let Some((tray_raw, exclude_raw)) = target else {
                continue;
            };
            let tray = HWND(tray_raw as *mut _);
            let measured = automation
                .as_ref()
                .and_then(|uia| unsafe { scan_taskbar_once(uia, tray, exclude_raw) });

            let mut changed = false;
            let mut settled = false;
            if let Some((bar, raw_merged)) = measured {
                let key = (tray_raw, exclude_raw, bar);
                let same_key = prev_key == Some(key);
                settled = same_key && prev_raw == raw_merged;
                let shown = if same_key {
                    let mut combined = Vec::with_capacity(prev_raw.len() + raw_merged.len());
                    combined.extend_from_slice(&prev_raw);
                    combined.extend_from_slice(&raw_merged);
                    merge_intervals(&combined, bar.left, bar.right)
                } else {
                    raw_merged.clone()
                };
                prev_key = Some(key);
                prev_raw = raw_merged;

                let next_snap = TaskbarSnapshot {
                    tray: tray_raw,
                    bar,
                    occupied: shown,
                };
                let mut guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
                if guard.target == Some((tray_raw, exclude_raw))
                    && guard.snapshot.as_ref() != Some(&next_snap)
                {
                    guard.snapshot = Some(next_snap);
                    changed = true;
                    if guard.overlay_thread != 0 {
                        // SAFETY: wakes the overlay thread to re-layout against the new room.
                        let _ = unsafe {
                            PostThreadMessageW(
                                guard.overlay_thread,
                                WM_TASKBAR_ROOM,
                                WPARAM(0),
                                LPARAM(0),
                            )
                        };
                    }
                }
            }

            if changed || !settled {
                settled_streak = 0;
                idle = SCAN_FAST;
            } else {
                settled_streak = settled_streak.saturating_add(1);
                if settled_streak >= 3 {
                    idle = (idle * 2).min(SCAN_MAX);
                }
            }

            let mut guard = s.state.lock().unwrap_or_else(|e| e.into_inner());
            let until = Instant::now() + idle;
            while guard.target.is_some() && !guard.nudge {
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                let (next_guard, res) = s
                    .wake
                    .wait_timeout(guard, left)
                    .unwrap_or_else(|e| e.into_inner());
                guard = next_guard;
                if res.timed_out() {
                    break;
                }
            }
            if guard.nudge {
                idle = SCAN_FAST;
            }
        }
    }

    /// What a window shows, kept for lines to light up in: a frame moves
    /// the cut by the pixels it passed, and draws nothing.
    struct Frame {
        memory: HDC,
        bitmap: HBITMAP,
        /// What `memory` held before `bitmap`.
        previous: HGDIOBJ,
        bits: *mut u32,
        width: i32,
        height: i32,
        /// The whole frame with every line that lights up lit, and faded.
        bright: Vec<u32>,
        faded: Vec<u32>,
        lit: Vec<LitLine>,
    }

    /// A line that lights up, in a frame.
    struct LitLine {
        /// Of the overlay's lines.
        index: usize,
        top: i32,
        bottom: i32,
        /// Where each character starts, and then where the last ends.
        edges: Vec<f32>,
        /// Where the cut stands.
        at: i32,
    }

    impl LitLine {
        /// Where the cut stands at `place`.
        fn cut(&self, place: f64) -> i32 {
            let last = self.edges.len().saturating_sub(1);
            let whole = (place.max(0.0) as usize).min(last);
            let start = self.edges.get(whole).copied().unwrap_or(0.0);
            let end = self.edges.get(whole + 1).copied().unwrap_or(start);
            (start + (end - start) * place.max(0.0).fract() as f32).round() as i32
        }
    }

    impl Drop for Frame {
        fn drop(&mut self) {
            // SAFETY: this frame's own device context and bitmap.
            unsafe {
                SelectObject(self.memory, self.previous);
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                let _ = DeleteDC(self.memory);
            }
        }
    }

    /// Move each lit line's cut to where it is now, and show only what it
    /// passed.
    unsafe fn advance(window: &mut Window) {
        let (hwnd, overlay) = (window.hwnd, &window.overlay);
        let Some(frame) = &mut window.frame else {
            return;
        };
        let (width, height) = (frame.width, frame.height);
        // SAFETY: the bitmap's pixels, top row first, as `draw` made them.
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(frame.bits, (width * height) as usize) };
        for line in &mut frame.lit {
            let Some(place) = overlay.lines.get(line.index).and_then(Line::lit_now) else {
                continue;
            };
            let to = line.cut(place).clamp(0, width);
            if to == line.at {
                continue;
            }
            let (left, right) = (to.min(line.at), to.max(line.at));
            let source = if to > line.at {
                &frame.bright
            } else {
                &frame.faded
            };
            for row in line.top..line.bottom {
                let span = (row * width + left) as usize..(row * width + right) as usize;
                pixels[span.clone()].copy_from_slice(&source[span]);
            }
            line.at = to;
            let dirty = RECT {
                left,
                top: line.top,
                right,
                bottom: line.bottom,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let origin = POINT { x: 0, y: 0 };
            let info = UPDATELAYEREDWINDOWINFO {
                cbSize: std::mem::size_of::<UPDATELAYEREDWINDOWINFO>() as u32,
                psize: &size,
                hdcSrc: frame.memory,
                pptSrc: &origin,
                pblend: &blend,
                dwFlags: ULW_ALPHA,
                prcDirty: &dirty,
                ..Default::default()
            };
            // SAFETY: a window of this thread, its frame, and locals that
            // outlive the call.
            let _ = unsafe { UpdateLayeredWindowIndirect(hwnd, &info) };
        }
    }

    /// A timer while a line on a route has somewhere to go.
    unsafe fn keep_moving(window: &mut Window) {
        let moving = window
            .overlay
            .lines
            .iter()
            .any(|line| line.route.as_ref().is_some_and(Route::moving));
        if moving == window.lit_timer {
            return;
        }
        // SAFETY: a window of this thread.
        unsafe {
            if moving {
                SetTimer(Some(window.hwnd), LIT_TIMER, LIT_EVERY, None);
            } else {
                let _ = KillTimer(Some(window.hwnd), LIT_TIMER);
            }
        }
        window.lit_timer = moving;
    }

    impl Window {
        /// The button at a point in the window, in pixels.
        fn button_at(&self, x: i32, y: i32) -> Option<usize> {
            self.buttons
                .iter()
                .find(|(rect, _)| {
                    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
                })
                .map(|(_, index)| *index)
        }

        /// Arm the hover poll timer once; never reset an already-running timer
        /// so high-frequency redraws cannot starve `WM_TIMER(HOVER_TIMER)`.
        unsafe fn ensure_hover_timer(&mut self) {
            if !self.hover_timer {
                unsafe { SetTimer(Some(self.hwnd), HOVER_TIMER, HOVER_EVERY, None) };
                self.hover_timer = true;
            }
        }

        unsafe fn stop_hover_timer(&mut self) {
            if self.hover_timer {
                let _ = unsafe { KillTimer(Some(self.hwnd), HOVER_TIMER) };
                self.hover_timer = false;
            }
        }

        unsafe fn ensure_taskbar_timer(&mut self) {
            if !self.taskbar_timer {
                unsafe { SetTimer(Some(self.hwnd), TASKBAR_TIMER, 1000, None) };
                self.taskbar_timer = true;
            }
        }

        unsafe fn stop_taskbar_timer(&mut self) {
            if self.taskbar_timer {
                let _ = unsafe { KillTimer(Some(self.hwnd), TASKBAR_TIMER) };
                self.taskbar_timer = false;
            }
        }

        unsafe fn stop_lit_timer(&mut self) {
            if self.lit_timer {
                let _ = unsafe { KillTimer(Some(self.hwnd), LIT_TIMER) };
                self.lit_timer = false;
            }
        }

        unsafe fn stop_all_timers(&mut self) {
            unsafe {
                self.stop_taskbar_timer();
                self.stop_hover_timer();
                self.stop_lit_timer();
            }
        }
    }

    thread_local! {
        static WINDOWS: RefCell<HashMap<String, Window>> = RefCell::new(HashMap::new());
        static FACTORIES: RefCell<Option<(ID2D1Factory, IDWriteFactory)>> = const { RefCell::new(None) };
    }

    /// Do `act` with the window `hwnd` and the factories, unless they are
    /// busy further up this thread.
    fn with_window<T>(
        hwnd: HWND,
        act: impl FnOnce(&mut Window, &ID2D1Factory, &IDWriteFactory) -> T,
    ) -> Option<T> {
        FACTORIES.with(|factories| {
            let factories = factories.try_borrow().ok()?;
            let (d2d, dwrite) = factories.as_ref()?;
            WINDOWS.with(|windows| {
                let mut windows = windows.try_borrow_mut().ok()?;
                let window = windows.values_mut().find(|window| window.hwnd == hwnd)?;
                Some(act(window, d2d, dwrite))
            })
        })
    }

    /// The pointer in the window, in pixels.
    fn pointer(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
        let mut point = POINT {
            x: (lparam.0 & 0xffff) as i16 as i32,
            y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
        };
        // SAFETY: the window this procedure serves.
        let _ = unsafe { ScreenToClient(hwnd, &mut point) };
        (point.x, point.y)
    }

    extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            // Its buttons take clicks. Unlocked, the rest drags the window;
            // locked, the window lets clicks through and never asks.
            WM_NCHITTEST => {
                let (x, y) = pointer(hwnd, lparam);
                let hit = with_window(hwnd, |window, _, _| match window.button_at(x, y) {
                    Some(_) if window.over => HTCLIENT as isize,
                    _ if window.overlay.locked => HTTRANSPARENT as isize,
                    _ => HTCAPTION as isize,
                });
                LRESULT(hit.unwrap_or(HTCAPTION as isize))
            }
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_LBUTTONDOWN | WM_LBUTTONUP => {
                let (x, y) = (
                    (lparam.0 & 0xffff) as i16 as i32,
                    ((lparam.0 >> 16) & 0xffff) as i16 as i32,
                );
                let pressed = with_window(hwnd, |window, _, _| {
                    let at = window.button_at(x, y);
                    if message == WM_LBUTTONDOWN {
                        window.pressed = at;
                        return None;
                    }
                    let pressed = window.pressed.take().filter(|&down| at == Some(down))?;
                    Some((
                        window.id.clone(),
                        window.overlay.buttons.get(pressed)?.id.clone(),
                    ))
                });
                if let Some(Some((overlay, button))) = pressed {
                    super::press(&overlay, &button);
                }
                LRESULT(0)
            }
            WM_ENTERSIZEMOVE => {
                with_window(hwnd, |window, _, _| {
                    window.dragging = true;
                });
                LRESULT(0)
            }
            WM_EXITSIZEMOVE => {
                let mut rect = RECT::default();
                // SAFETY: the window this procedure serves.
                let got_rect = unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok();
                with_window(hwnd, |window, _, _| {
                    window.dragging = false;
                    if !got_rect {
                        return;
                    }
                    let place = ((rect.left + rect.right) as f64 / 2.0, rect.bottom as f64);
                    window.place = Some(place);
                    super::save_place(&window.id, place.0, place.1);
                    let allowed = window.visible
                        && !window.overlay.locked
                        && window.overlay.place == Place::Free;
                    if allowed {
                        super::update_rect(
                            &window.id,
                            Some((
                                rect.left as f64,
                                rect.top as f64,
                                (rect.right - rect.left) as f64,
                                (rect.bottom - rect.top) as f64,
                            )),
                        );
                    }
                });
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == LIT_TIMER => {
                with_window(hwnd, |window, _, _| {
                    // SAFETY: a window of this thread.
                    unsafe { advance(window) };
                });
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == HOVER_TIMER => {
                with_window(hwnd, |window, d2d, dwrite| {
                    if window.dragging {
                        return;
                    }
                    if window.visible && window.overlay.place == Place::Free {
                        // SAFETY: reads extended style of this thread's window.
                        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
                        if (ex & WS_EX_TOPMOST.0) == 0 {
                            let _ = unsafe { draw(window, d2d, dwrite, false) };
                            return;
                        }
                    }
                    let mut cursor = POINT::default();
                    let mut rect = RECT::default();
                    // SAFETY: reads the pointer and this thread's window.
                    let found = unsafe {
                        GetCursorPos(&mut cursor).is_ok() && GetWindowRect(hwnd, &mut rect).is_ok()
                    };
                    if !found {
                        return;
                    }
                    let over = cursor.x >= rect.left
                        && cursor.x < rect.right
                        && cursor.y >= rect.top
                        && cursor.y < rect.bottom;
                    let hot = if over {
                        window.button_at(cursor.x - rect.left, cursor.y - rect.top)
                    } else {
                        None
                    };
                    if (over, hot) != (window.over, window.hot) {
                        window.over = over;
                        window.hot = hot;
                        // SAFETY: draws a window of this thread.
                        let _ = unsafe { draw(window, d2d, dwrite, false) };
                    }
                });
                LRESULT(0)
            }
            WM_TIMER => {
                with_window(hwnd, |window, d2d, dwrite| {
                    if !window.visible || window.overlay.place != Place::Taskbar {
                        return;
                    }
                    // SAFETY: reads the shell's window handle and redraws if
                    // the taskbar restarted or its free room changed.
                    unsafe {
                        let tray = find_taskbar();
                        let desired = tray.and_then(|t| {
                            let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
                            live_taskbar_room(
                                t,
                                dwrite,
                                &window.overlay.lines,
                                scale,
                                2.0 * scale,
                                window.room,
                            )
                        });
                        if tray != window.parent || desired != window.room {
                            let _ = draw(window, d2d, dwrite, false);
                        } else if window.parent.is_some() {
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND_TOP),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                    }
                });
                LRESULT(0)
            }
            // SAFETY: the default handling of a window of this thread.
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    thread_local! {
        static INBOX: RefCell<Option<mpsc::Receiver<Message>>> = const { RefCell::new(None) };
        static INSTANCE_HANDLE: std::cell::Cell<isize> = const { std::cell::Cell::new(0) };
        static FLUENT_ICONS: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    }

    fn current_instance() -> windows::Win32::Foundation::HINSTANCE {
        windows::Win32::Foundation::HINSTANCE(INSTANCE_HANDLE.with(|i| i.get()) as *mut _)
    }

    fn drain_inbox() {
        let mut pending = Vec::new();
        INBOX.with(|inbox| {
            if let Some(rx) = inbox.borrow().as_ref() {
                while let Ok(next) = rx.try_recv() {
                    pending.push(next);
                }
            }
        });
        if pending.is_empty() {
            return;
        }
        let coalesced = coalesce_win_ops(pending);
        FACTORIES.with(|factories| {
            if let Some((d2d, dwrite)) = factories.borrow().as_ref() {
                for next in coalesced {
                    // SAFETY: overlay thread owns these factories and windows.
                    unsafe { apply(next, d2d, dwrite) };
                }
            }
        });
    }

    extern "system" fn wake_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_APP {
            drain_inbox();
            return LRESULT(0);
        }
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    fn run(messages: mpsc::Receiver<Message>, ready: mpsc::Sender<isize>) {
        // SAFETY: this thread's own settings, window class and factories.
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            let instance = GetModuleHandleW(None).unwrap_or_default();
            INSTANCE_HANDLE.with(|i| i.set(instance.0 as isize));
            INBOX.with(|inbox| *inbox.borrow_mut() = Some(messages));
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("BernOverlay"),
                ..Default::default()
            };
            RegisterClassExW(&class);
            let wake_class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wake_proc),
                hInstance: instance.into(),
                lpszClassName: w!("BernOverlayWake"),
                ..Default::default()
            };
            RegisterClassExW(&wake_class);
            let Ok(wake_hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BernOverlayWake"),
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance.into()),
                None,
            ) else {
                return;
            };
            let Ok(d2d) =
                D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
            else {
                return;
            };
            let Ok(dwrite) = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)
            else {
                return;
            };
            FACTORIES.with(|factories| *factories.borrow_mut() = Some((d2d, dwrite)));
            let thread_id = GetCurrentThreadId();
            set_scanner_thread(thread_id);
            let _ = ready.send(wake_hwnd.0 as isize);
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                if message.hwnd.is_invalid() && message.message == WM_TASKBAR_ROOM {
                    WINDOWS.with(|windows| {
                        let mut windows = windows.borrow_mut();
                        FACTORIES.with(|factories| {
                            if let Some((d2d, dwrite)) = factories.borrow().as_ref() {
                                for window in windows.values_mut() {
                                    if window.visible && window.overlay.place == Place::Taskbar {
                                        let _ = draw(window, d2d, dwrite, false);
                                    }
                                }
                            }
                        });
                    });
                    continue;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Create an overlay window with `WS_EX_TOPMOST` baked in for free overlays:
    /// on Windows 11 a non-foreground process cannot promote a non-topmost
    /// window via `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`.
    unsafe fn create_overlay_window(place: Place) -> HWND {
        let mut ex = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        if place == Place::Free {
            ex |= WS_EX_TOPMOST;
        }
        // SAFETY: a window of the class this thread registered.
        unsafe {
            CreateWindowExW(
                ex,
                w!("BernOverlay"),
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(current_instance()),
                None,
            )
        }
        .unwrap_or_default()
    }

    unsafe fn apply(message: Message, d2d: &ID2D1Factory, dwrite: &IDWriteFactory) {
        match message {
            Message::Hide(id) => WINDOWS.with(|windows| {
                let mut windows = windows.borrow_mut();
                if let Some(window) = windows.get_mut(&id) {
                    window.visible = false;
                    window.over = false;
                    window.hot = None;
                    window.dragging = false;
                    super::update_rect(&id, None);
                    // SAFETY: a window of this thread.
                    unsafe {
                        window.stop_all_timers();
                        let _ = ShowWindow(window.hwnd, SW_HIDE);
                    }
                }
                refresh_scanner(&windows);
            }),
            Message::Lit(id, overlay, style) => {
                let moved = WINDOWS.with(|windows| {
                    let mut windows = windows.borrow_mut();
                    let Some(window) = windows.get_mut(&id).filter(|window| window.frame.is_some())
                    else {
                        return false;
                    };
                    window.overlay = overlay.clone();
                    // SAFETY: a window of this thread.
                    unsafe {
                        advance(window);
                        keep_moving(window);
                    }
                    true
                });
                if !moved {
                    // SAFETY: as this call's own.
                    unsafe { apply(Message::Show(id, overlay, style), d2d, dwrite) };
                }
            }
            Message::Color(id, overlay, style) => {
                let repainted = WINDOWS.with(|windows| {
                    let mut windows = windows.borrow_mut();
                    let Some(window) = windows
                        .get_mut(&id)
                        .filter(|window| window.visible && window.frame.is_some())
                    else {
                        return false;
                    };
                    window.overlay = overlay.clone();
                    window.style = style;
                    // SAFETY: repaints pixels in place on a window of this thread
                    // without resetting hover timers, window styles, or drag position.
                    let _ = unsafe { draw(window, d2d, dwrite, true) };
                    true
                });
                if !repainted {
                    // SAFETY: as this call's own.
                    unsafe { apply(Message::Show(id, overlay, style), d2d, dwrite) };
                }
            }
            Message::Show(id, overlay, style) => WINDOWS.with(|windows| {
                let mut windows = windows.borrow_mut();
                {
                    let initial_place = overlay.place;
                    let window = windows.entry(id.clone()).or_insert_with(|| {
                        let hwnd = unsafe { create_overlay_window(initial_place) };
                        Window {
                            id: id.clone(),
                            hwnd,
                            overlay: Overlay::default(),
                            place: super::saved_place(&id),
                            over: false,
                            hot: None,
                            pressed: None,
                            buttons: Vec::new(),
                            frame: None,
                            parent: None,
                            visible: false,
                            room: None,
                            style: TextStyle::default(),
                            hover_timer: false,
                            taskbar_timer: false,
                            lit_timer: false,
                            dragging: false,
                        }
                    });
                    window.overlay = overlay;
                    window.style = style;
                    window.visible = true;
                }
                refresh_scanner(&windows);
                if let Some(window) = windows.get_mut(&id) {
                    // SAFETY: draws a window of this thread.
                    let _ = unsafe { draw(window, d2d, dwrite, false) };
                }
            }),
        }
    }

    /// As wide as a line is laid out when nothing limits it, in pixels.
    const UNLIMITED: f32 = 8192.0;

    /// The text of a line laid out on one line, and its size in pixels.
    /// Wider than `limit`, its end gives way to an ellipsis.
    unsafe fn layout(
        dwrite: &IDWriteFactory,
        line: &super::Line,
        scale: f32,
        limit: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        let weight = if line.bold {
            DWRITE_FONT_WEIGHT_BOLD
        } else {
            DWRITE_FONT_WEIGHT_NORMAL
        };
        // SAFETY: DirectWrite objects made and used on this thread.
        unsafe {
            let format = dwrite
                .CreateTextFormat(
                    w!("Microsoft YaHei UI"),
                    None,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    line.size as f32 * scale,
                    w!(""),
                )
                .ok()?;
            if limit < UNLIMITED {
                format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP).ok()?;
                let ellipsis = dwrite.CreateEllipsisTrimmingSign(&format).ok()?;
                let trimming = DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    delimiter: 0,
                    delimiterCount: 0,
                };
                format.SetTrimming(&trimming, &ellipsis).ok()?;
            }
            let text: Vec<u16> = line.text.encode_utf16().collect();
            let layout = dwrite
                .CreateTextLayout(&text, &format, limit.max(1.0), UNLIMITED)
                .ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            Some((
                layout,
                metrics.widthIncludingTrailingWhitespace.min(limit),
                metrics.height,
            ))
        }
    }

    /// A symbol of the bar laid out in the system's icon font, and its size
    /// in pixels: Segoe Fluent Icons on Windows 11, Segoe MDL2 Assets
    /// before.
    unsafe fn glyph(
        dwrite: &IDWriteFactory,
        symbol: &str,
        scale: f32,
    ) -> Option<(IDWriteTextLayout, f32, f32)> {
        let code = GLYPHS.iter().find(|(name, _)| *name == symbol)?.1;
        // SAFETY: DirectWrite objects made and used on this thread.
        unsafe {
            let fluent = match FLUENT_ICONS.with(|cached| cached.get()) {
                Some(has) => has,
                None => {
                    let mut fonts: Option<IDWriteFontCollection> = None;
                    dwrite.GetSystemFontCollection(&mut fonts, false).ok()?;
                    let (mut index, mut exists) = (0u32, BOOL(0));
                    let has = fonts
                        .and_then(|fonts| {
                            fonts
                                .FindFamilyName(w!("Segoe Fluent Icons"), &mut index, &mut exists)
                                .ok()
                        })
                        .is_some()
                        && exists.as_bool();
                    FLUENT_ICONS.with(|cached| cached.set(Some(has)));
                    has
                }
            };
            let family: PCWSTR = if fluent {
                w!("Segoe Fluent Icons")
            } else {
                w!("Segoe MDL2 Assets")
            };
            let format = dwrite
                .CreateTextFormat(
                    family,
                    None,
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    13.0 * scale,
                    w!(""),
                )
                .ok()?;
            let layout = dwrite
                .CreateTextLayout(&[code], &format, 256.0, 256.0)
                .ok()?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics).ok()?;
            Some((
                layout,
                metrics.widthIncludingTrailingWhitespace,
                metrics.height,
            ))
        }
    }

    fn find_taskbar() -> Option<HWND> {
        // SAFETY: queries the shell's primary taskbar window handle.
        unsafe { FindWindowW(w!("Shell_TrayWnd"), None).ok() }.filter(|&h| !h.0.is_null())
    }

    unsafe fn has_visible_child(parent: HWND, class_name: PCWSTR) -> bool {
        let mut child = HWND::default();
        loop {
            // SAFETY: enumerates child windows of `parent`.
            let Ok(next) = (unsafe {
                FindWindowExW(
                    Some(parent),
                    (!child.0.is_null()).then_some(child),
                    class_name,
                    None,
                )
            }) else {
                break;
            };
            if next.0.is_null() {
                break;
            }
            // SAFETY: checks visibility of a live child window handle.
            if unsafe { IsWindowVisible(next) }.as_bool() {
                return true;
            }
            child = next;
        }
        false
    }

    unsafe fn scan_taskbar_once(
        automation: &IUIAutomation,
        tray: HWND,
        exclude_raw: isize,
    ) -> Option<(Room, Vec<(i32, i32)>)> {
        // SAFETY: runs on the background UIA scanner thread against `tray`.
        unsafe {
            if !IsWindow(Some(tray)).as_bool() {
                return None;
            }
            let mut bar = RECT::default();
            GetWindowRect(tray, &mut bar).ok()?;
            if bar.right <= bar.left || bar.bottom <= bar.top {
                return None;
            }
            let bar_room = Room {
                left: bar.left,
                right: bar.right,
                top: bar.top,
                bottom: bar.bottom,
            };
            let mut raw = Vec::new();
            if let Some(span) = notify_span(tray, &bar) {
                raw.push(span);
            }

            let is_xaml = has_visible_child(
                tray,
                w!("Windows.UI.Composition.DesktopWindowContentBridge"),
            );
            if let Ok(root) = automation.ElementFromHandle(tray)
                && let Ok(true_cond) = automation.CreateTrueCondition()
            {
                let _ = collect_occupied(&true_cond, &root, &bar, exclude_raw, is_xaml, &mut raw);
            }
            if raw.is_empty() {
                return None;
            }
            Some((bar_room, merge_intervals(&raw, bar.left, bar.right)))
        }
    }

    unsafe fn collect_occupied(
        true_cond: &IUIAutomationCondition,
        element: &IUIAutomationElement,
        bar: &RECT,
        exclude_raw: isize,
        is_xaml: bool,
        acc: &mut Vec<(i32, i32)>,
    ) -> Option<i32> {
        // SAFETY: UIA calls on the background scanner thread.
        unsafe {
            let kids = element.FindAll(TreeScope_Children, true_cond).ok()?;
            let count = kids.Length().unwrap_or(0);
            let bar_width = bar.right - bar.left;
            for index in 0..count {
                let Ok(child) = kids.GetElement(index) else {
                    continue;
                };
                if exclude_raw != 0
                    && child
                        .CurrentNativeWindowHandle()
                        .is_ok_and(|h| h.0 as isize == exclude_raw)
                {
                    continue;
                }
                if child.CurrentIsOffscreen().is_ok_and(|b| b.as_bool()) {
                    continue;
                }
                let Ok(rect) = child.CurrentBoundingRectangle() else {
                    continue;
                };
                let width = rect.right - rect.left;
                if width <= 0
                    || rect.bottom <= rect.top
                    || rect.bottom <= bar.top
                    || rect.top >= bar.bottom
                {
                    continue;
                }
                let cls = child
                    .CurrentClassName()
                    .map(|b| b.to_string())
                    .unwrap_or_default();
                let is_legacy = matches!(
                    cls.as_str(),
                    "ReBarWindow32" | "WorkerW" | "MSTaskSwWClass" | "MSTaskListWClass"
                );
                if is_legacy {
                    if is_xaml {
                        continue;
                    }
                    if collect_occupied(true_cond, &child, bar, exclude_raw, is_xaml, acc).is_none()
                    {
                        acc.push((rect.left, rect.right));
                    }
                    continue;
                }

                let kind = child.CurrentControlType().unwrap_or(UIA_CONTROLTYPE_ID(0));
                if LEAF_OCCUPYING.contains(&kind) && width <= bar_width / 2 {
                    acc.push((rect.left, rect.right));
                    continue;
                }

                // Recurse into containers (even sub-full-width ones such as
                // TaskbarFrame or Widgets wrappers) so empty spans stay free.
                match collect_occupied(true_cond, &child, bar, exclude_raw, is_xaml, acc) {
                    None if width <= bar_width / 2 => acc.push((rect.left, rect.right)),
                    Some(0) if width <= bar_width / 2 && !CONTAINER_TYPES.contains(&kind) => {
                        acc.push((rect.left, rect.right));
                    }
                    _ => {}
                }
            }
            Some(count)
        }
    }

    fn measure_lines_width(dwrite: &IDWriteFactory, lines: &[Line], scale: f32) -> f32 {
        lines
            .iter()
            .filter(|line| !line.text.is_empty())
            .filter_map(|line| unsafe { layout(dwrite, line, scale, UNLIMITED) })
            .map(|(_, w, _)| w)
            .fold(0.0, f32::max)
    }

    fn min_taskbar_width(lines: &[Line], scale: f32, margin: f32) -> i32 {
        let min_glyph = lines
            .iter()
            .filter(|line| !line.text.is_empty())
            .map(|line| line.size as f32 * scale)
            .fold(0.0, f32::max);
        (min_glyph + 2.0 * margin).ceil() as i32
    }

    unsafe fn notify_span(tray: HWND, bar: &RECT) -> Option<(i32, i32)> {
        // SAFETY: queries the shell's notification area rect.
        unsafe {
            let notify = FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), None).ok()?;
            if notify.0.is_null() || !IsWindowVisible(notify).as_bool() {
                return None;
            }
            let mut area = RECT::default();
            if GetWindowRect(notify, &mut area).is_ok()
                && area.right > area.left
                && area.bottom > bar.top
                && area.top < bar.bottom
            {
                Some((area.left, area.right))
            } else {
                None
            }
        }
    }

    unsafe fn live_taskbar_room(
        tray: HWND,
        dwrite: &IDWriteFactory,
        lines: &[Line],
        scale: f32,
        margin: f32,
        current: Option<Room>,
    ) -> Option<Room> {
        let snap = latest_taskbar_snapshot(tray)?;
        let bar_rect = RECT {
            left: snap.bar.left,
            top: snap.bar.top,
            right: snap.bar.right,
            bottom: snap.bar.bottom,
        };
        let want = measure_lines_width(dwrite, lines, scale) + 2.0 * margin;
        let tb_gap = (TASKBAR_GAP as f32 * scale).round() as i32;
        let min_w = min_taskbar_width(lines, scale, margin);
        // SAFETY: reads the live notification area rect so newly added tray
        // icons shrink available room immediately without waiting for UIA.
        if let Some(notify) = unsafe { notify_span(tray, &bar_rect) } {
            let mut occupied = snap.occupied.clone();
            occupied.push(notify);
            pick_taskbar_room(
                snap.bar,
                &occupied,
                tb_gap,
                min_w,
                want.ceil() as i32,
                current,
            )
        } else {
            pick_taskbar_room(
                snap.bar,
                &snap.occupied,
                tb_gap,
                min_w,
                want.ceil() as i32,
                current,
            )
        }
    }

    unsafe fn sync_parent(window: &mut Window, tray: Option<HWND>) {
        // SAFETY: updates Win32 parent/style only when switching mode or when
        // the taskbar window handle changes.
        unsafe {
            if window.overlay.place == Place::Taskbar {
                let valid_tray = tray.filter(|&t| IsWindow(Some(t)).as_bool());
                let Some(tray) = valid_tray else {
                    window.parent = None;
                    return;
                };
                let style = GetWindowLongPtrW(window.hwnd, GWL_STYLE) as u32;
                let current_parent = GetParent(window.hwnd).ok();
                if (style & WS_CHILD.0) == 0 || current_parent != Some(tray) {
                    window.stop_all_timers();
                    let _ = SetWindowPos(
                        window.hwnd,
                        Some(HWND_NOTOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    let _ = SetParent(window.hwnd, Some(tray));
                    let new_style = (style & !WS_POPUP.0) | WS_CHILD.0;
                    SetWindowLongPtrW(window.hwnd, GWL_STYLE, new_style as isize);
                    let _ = SetWindowPos(
                        window.hwnd,
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                    window.parent = Some(tray);
                    window.room = None;
                }
            } else {
                let style = GetWindowLongPtrW(window.hwnd, GWL_STYLE) as u32;
                let ex = GetWindowLongPtrW(window.hwnd, GWL_EXSTYLE) as u32;
                let is_child = (style & WS_CHILD.0) != 0;
                let missing_topmost = (ex & WS_EX_TOPMOST.0) == 0;
                if (is_child || missing_topmost) && !window.dragging {
                    window.stop_all_timers();
                    let old = window.hwnd;
                    window.frame = None;
                    window.hwnd = create_overlay_window(Place::Free);
                    if !old.0.is_null() {
                        let _ = DestroyWindow(old);
                    }
                }
                window.parent = None;
                window.room = None;
            }
        }
    }

    type Reading = (IDWriteTextLayout, f32, f32);
    type DrawnLine = (
        IDWriteTextLayout,
        f32,
        f32,
        [u8; 4],
        Option<usize>,
        Vec<Reading>,
        f32,
    );

    unsafe fn draw(
        window: &mut Window,
        d2d: &ID2D1Factory,
        dwrite: &IDWriteFactory,
        color_only: bool,
    ) -> windows::core::Result<()> {
        // SAFETY: Win32, Direct2D and DirectWrite calls on this thread's
        // window and its own device contexts and bitmap.
        unsafe {
            let free = window.overlay.place == Place::Free;
            if !color_only {
                let tray = if free { None } else { find_taskbar() };
                sync_parent(window, tray);
            }
            let hwnd = window.hwnd;

            let dpi = GetDpiForWindow(hwnd).max(96);
            let scale = dpi as f32 / 96.0;
            // On the taskbar the lines fill its height, with no room around.
            let (margin, gap) = if free {
                (MARGIN * scale, GAP * scale)
            } else {
                (2.0 * scale, 0.0)
            };
            let room = if free {
                None
            } else if color_only && window.room.is_some() {
                window.room
            } else if let Some(tray) = window.parent {
                live_taskbar_room(
                    tray,
                    dwrite,
                    &window.overlay.lines,
                    scale,
                    margin,
                    window.room,
                )
            } else {
                None
            };
            if room != window.room {
                if let Some(room) = room {
                    eprintln!(
                        "bern overlay: taskbar room {}..{} of {}..{}",
                        room.left, room.right, room.top, room.bottom
                    );
                }
                window.room = room;
            }
            if !free && room.is_none() {
                window.stop_hover_timer();
                window.stop_lit_timer();
                let _ = ShowWindow(hwnd, SW_HIDE);
                window.frame = None;
                window.ensure_taskbar_timer();
                return Ok(());
            }
            // On the taskbar a line is as wide as its room; one that has not
            // room for a character is left out.
            let limit = room.map_or(UNLIMITED, |room| {
                (room.right - room.left) as f32 - 2.0 * margin
            });
            let lines: Vec<DrawnLine> = window
                .overlay
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| !line.text.is_empty() && limit >= line.size as f32 * scale)
                .filter_map(|(index, line)| {
                    layout(dwrite, line, scale, limit).map(|(layout, w, h)| {
                        let spans = line.ruby_spans();
                        let band = if spans.is_empty() {
                            0.0
                        } else {
                            line.size as f32 * scale * 0.7
                        };
                        let units: Vec<u32> = line
                            .text
                            .chars()
                            .scan(0, |at, c| {
                                let start = *at;
                                *at += c.len_utf16() as u32;
                                Some(start)
                            })
                            .chain(std::iter::once(line.text.encode_utf16().count() as u32))
                            .collect();
                        let mut readings = Vec::new();
                        for (start, end, text) in spans {
                            let (mut left, mut right, mut y) = (0.0, 0.0, 0.0);
                            let mut hit = DWRITE_HIT_TEST_METRICS::default();
                            if layout
                                .HitTestTextPosition(
                                    units[start],
                                    false,
                                    &mut left,
                                    &mut y,
                                    &mut hit,
                                )
                                .is_err()
                                || layout
                                    .HitTestTextPosition(
                                        units[end], false, &mut right, &mut y, &mut hit,
                                    )
                                    .is_err()
                            {
                                continue;
                            }
                            // Over what the ellipsis cut off.
                            if left.max(right) > w + 0.5 {
                                continue;
                            }
                            let available = (right - left).abs().max(1.0);
                            let mut style = line.clone();
                            style.text = text;
                            style.size *= 0.45;
                            if let Some((_, measured, _)) =
                                self::layout(dwrite, &style, scale, UNLIMITED)
                            {
                                style.size *= (available / measured.max(1.0)).min(1.0) as f64;
                                if let Some((reading, wide, high)) =
                                    self::layout(dwrite, &style, scale, UNLIMITED)
                                {
                                    readings.push((
                                        reading,
                                        left.min(right) + (available - wide) / 2.0,
                                        (band - high).max(0.0),
                                    ));
                                }
                            }
                        }
                        (
                            layout,
                            w,
                            h + band,
                            line.color,
                            line.lights().then_some(index),
                            readings,
                            band,
                        )
                    })
                })
                .collect();
            if lines.is_empty() {
                window.stop_hover_timer();
                window.stop_lit_timer();
                let _ = ShowWindow(hwnd, SW_HIDE);
                window.frame = None;
                super::update_rect(&window.id, None);
                // Still on the taskbar, it checks again if the layout changes.
                if !free {
                    window.ensure_taskbar_timer();
                }
                return Ok(());
            }
            // Every button has its slot; locked, only those that unlock show.
            let slots = if free {
                window.overlay.buttons.len()
            } else {
                0
            };
            let (button, button_gap) = (BUTTON * scale, BUTTON_GAP * scale);
            let bar_width = slots as f32 * button + slots.saturating_sub(1) as f32 * button_gap;
            let bar_height = if slots == 0 {
                0.0
            } else {
                button + BAR_GAP * scale
            };
            let width = (lines.iter().map(|line| line.1).fold(bar_width, f32::max) + 2.0 * margin)
                .ceil() as i32;
            let height = (lines.iter().map(|line| line.2).sum::<f32>()
                + gap * (lines.len() - 1) as f32
                + bar_height
                + 2.0 * margin)
                .ceil() as i32;
            let first = (width as f32 - bar_width) / 2.0;
            window.buttons = (0..slots)
                .filter(|&index| {
                    !window.overlay.locked || window.overlay.buttons[index].while_locked
                })
                .map(|index| {
                    let left = first + index as f32 * (button + button_gap);
                    let rect = RECT {
                        left: left as i32,
                        top: margin as i32,
                        right: (left + button) as i32,
                        bottom: (margin + button) as i32,
                    };
                    (rect, index)
                })
                .collect();

            // Refresh hover/hot right before drawing whenever the overlay is
            // already on screen and not being dragged: Win32 treats `WM_TIMER`
            // as lower priority than posted `WM_APP` updates, so checking here
            // keeps hover controls and click-through responsive even under
            // continuous animation frames.
            if free && !window.dragging && IsWindowVisible(hwnd).as_bool() {
                let mut cursor = POINT::default();
                let mut rect = RECT::default();
                if GetCursorPos(&mut cursor).is_ok() && GetWindowRect(hwnd, &mut rect).is_ok() {
                    let over = cursor.x >= rect.left
                        && cursor.x < rect.right
                        && cursor.y >= rect.top
                        && cursor.y < rect.bottom;
                    window.over = over;
                    window.hot = if over {
                        window.button_at(cursor.x - rect.left, cursor.y - rect.top)
                    } else {
                        None
                    };
                }
            }

            let screen = GetDC(None);
            let memory = CreateCompatibleDC(Some(screen));
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(Some(memory), &info, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let previous = SelectObject(memory, HGDIOBJ(bitmap.0));
            let target = d2d.CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            })?;
            target.BindDC(
                memory,
                &RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )?;
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let lights = lines.iter().any(|line| line.4.is_some());
            let pixels = (width * height) as usize;
            let (mut bright, mut faded) = (Vec::new(), Vec::new());
            let mut lit = Vec::new();
            // What lights up is drawn lit, then faded: moving the cut then
            // draws nothing.
            for dim in [false, true].into_iter().take(if lights { 2 } else { 1 }) {
                lit.clear();
                target.BeginDraw();
                target.Clear(Some(&D2D1_COLOR_F {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                }));
                let dark = |a: f32| {
                    target.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a,
                        },
                        None,
                    )
                };
                // The plate, while the pointer is over an unlocked overlay or
                // always on the taskbar.
                if !window.overlay.locked && (window.over || !free) {
                    let plate = D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: 0.0,
                            top: 0.0,
                            right: width as f32,
                            bottom: height as f32,
                        },
                        radiusX: 12.0 * scale,
                        radiusY: 12.0 * scale,
                    };
                    target.FillRoundedRectangle(&plate, &dark(0.3)?);
                }
                if window.over {
                    let white = target.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        },
                        None,
                    )?;
                    for (rect, index) in &window.buttons {
                        let middle = Vector2 {
                            X: (rect.left + rect.right) as f32 / 2.0,
                            Y: (rect.top + rect.bottom) as f32 / 2.0,
                        };
                        let round = D2D1_ELLIPSE {
                            point: middle,
                            radiusX: button / 2.0,
                            radiusY: button / 2.0,
                        };
                        target.FillEllipse(
                            &round,
                            &dark(if window.hot == Some(*index) {
                                0.55
                            } else {
                                0.3
                            })?,
                        );
                        if let Some((layout, glyph_width, glyph_height)) =
                            glyph(dwrite, &window.overlay.buttons[*index].symbol, scale)
                        {
                            let at = Vector2 {
                                X: middle.X - glyph_width / 2.0,
                                Y: middle.Y - glyph_height / 2.0,
                            };
                            target.DrawTextLayout(at, &layout, &white, D2D1_DRAW_TEXT_OPTIONS_NONE);
                        }
                    }
                }
                let text_opacity = window.style.opacity as f32;
                let effect_alpha = match window.style.effect {
                    TextEffect::None => 0.0,
                    TextEffect::Shadow => 0.75 * text_opacity,
                    TextEffect::Outline => 0.9 * text_opacity,
                };
                let shadow = dark(effect_alpha)?;
                let offsets: &[(f32, f32)] = match window.style.effect {
                    TextEffect::None => &[],
                    TextEffect::Shadow => &[(1.0, 1.5), (0.0, 1.0), (1.0, 0.0), (-0.5, 1.0)],
                    TextEffect::Outline => &[
                        (-1.5, 0.0),
                        (1.5, 0.0),
                        (0.0, -1.5),
                        (0.0, 1.5),
                        (-1.0, -1.0),
                        (1.0, -1.0),
                        (-1.0, 1.0),
                        (1.0, 1.0),
                    ],
                };
                let mut top = margin + bar_height;
                for (layout, line_width, line_height, [r, g, b, a], index, readings, band) in &lines
                {
                    let left = (width as f32 - line_width) / 2.0;
                    let alpha = text_opacity
                        * if dim && index.is_some() {
                            super::FADED as f32
                        } else {
                            1.0
                        };
                    let brush = target.CreateSolidColorBrush(
                        &D2D1_COLOR_F {
                            r: *r as f32 / 255.0,
                            g: *g as f32 / 255.0,
                            b: *b as f32 / 255.0,
                            a: *a as f32 / 255.0 * alpha,
                        },
                        None,
                    )?;
                    for &(dx, dy) in offsets {
                        target.DrawTextLayout(
                            Vector2 {
                                X: left + dx * scale,
                                Y: top + band + dy * scale,
                            },
                            layout,
                            &shadow,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                    let at = Vector2 {
                        X: left,
                        Y: top + band,
                    };
                    target.DrawTextLayout(at, layout, &brush, D2D1_DRAW_TEXT_OPTIONS_NONE);
                    for (reading, x, y) in readings {
                        for &(dx, dy) in offsets {
                            target.DrawTextLayout(
                                Vector2 {
                                    X: left + x + dx * scale,
                                    Y: top + y + dy * scale,
                                },
                                reading,
                                &shadow,
                                D2D1_DRAW_TEXT_OPTIONS_NONE,
                            );
                        }
                        target.DrawTextLayout(
                            Vector2 {
                                X: left + x,
                                Y: top + y,
                            },
                            reading,
                            &brush,
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                    if let Some(index) = index {
                        let mut edges = Vec::new();
                        let (mut x, mut y) = (0.0, 0.0);
                        let mut metrics = DWRITE_HIT_TEST_METRICS::default();
                        let mut unit = 0;
                        for character in window.overlay.lines[*index].text.chars() {
                            layout.HitTestTextPosition(
                                unit,
                                false,
                                &mut x,
                                &mut y,
                                &mut metrics,
                            )?;
                            edges.push(left + x);
                            unit += character.len_utf16() as u32;
                        }
                        edges.push(left + x + metrics.width);
                        lit.push(LitLine {
                            index: *index,
                            top: top.floor() as i32,
                            bottom: ((top + line_height).ceil() as i32 + 2).min(height),
                            edges,
                            at: 0,
                        });
                    }
                    top += line_height + gap;
                }
                target.EndDraw(None, None)?;
                let _ = GdiFlush();
                if lights {
                    let drawn = std::slice::from_raw_parts(bits as *const u32, pixels);
                    if dim {
                        faded = drawn.to_vec();
                    } else {
                        bright = drawn.to_vec();
                    }
                }
            }
            // Everything that lights up starts out faded, as drawn last; the
            // cut then moves to where it is.
            let mut frame = Frame {
                memory,
                bitmap,
                previous,
                bits: bits as *mut u32,
                width,
                height,
                bright,
                faded,
                lit,
            };
            let shown = std::slice::from_raw_parts_mut(frame.bits, pixels);
            for line in &mut frame.lit {
                let place = window.overlay.lines[line.index]
                    .lit_now()
                    .unwrap_or(f64::MAX);
                line.at = line.cut(place).clamp(0, width);
                for row in line.top..line.bottom {
                    let span = (row * width) as usize..(row * width + line.at) as usize;
                    shown[span.clone()].copy_from_slice(&frame.bright[span]);
                }
            }

            let prev_size = window.frame.as_ref().map(|f| (f.width, f.height));
            let same_size = prev_size == Some((width, height));
            let keep_pos = (color_only && free && same_size) || (window.dragging && same_size);
            let dst_point = if keep_pos {
                None
            } else {
                let (left, top) = match window.overlay.place {
                    Place::Taskbar => room.map_or((0, 0), |room| {
                        let screen_x = room.right - width;
                        let screen_y = (room.top + room.bottom) / 2 - height / 2;
                        if let Some(tray) = window.parent {
                            let mut pt = POINT {
                                x: screen_x,
                                y: screen_y,
                            };
                            let _ = ScreenToClient(tray, &mut pt);
                            (pt.x, pt.y)
                        } else {
                            (screen_x, screen_y)
                        }
                    }),
                    Place::Free => {
                        let (x, y) = if window.dragging {
                            let mut rect = RECT::default();
                            if GetWindowRect(hwnd, &mut rect).is_ok() {
                                ((rect.left + rect.right) as f64 / 2.0, rect.bottom as f64)
                            } else {
                                window.place.unwrap_or((0.0, 0.0))
                            }
                        } else {
                            window.place.unwrap_or_else(|| {
                                let mut area = RECT::default();
                                let _ = SystemParametersInfoW(
                                    SPI_GETWORKAREA,
                                    0,
                                    Some(&mut area as *mut RECT as *mut _),
                                    Default::default(),
                                );
                                (
                                    (area.left + area.right) as f64 / 2.0,
                                    area.bottom as f64 - (area.bottom - area.top) as f64 * 0.1,
                                )
                            })
                        };
                        ((x - width as f64 / 2.0) as i32, (y - height as f64) as i32)
                    }
                };
                Some(POINT { x: left, y: top })
            };
            let mut style_changed = false;
            // Locked, clicks go through except over a button that shows.
            // Checked on color-only repaints too so hovering the unlock button
            // during high-rate color frames clears `WS_EX_TRANSPARENT` at once.
            let through = window.overlay.locked && !(window.over && window.hot.is_some());
            let old_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let new_style = if through {
                old_style | WS_EX_TRANSPARENT.0
            } else {
                old_style & !WS_EX_TRANSPARENT.0
            };
            if new_style != old_style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style as isize);
                style_changed = true;
            }
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let result = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                dst_point.as_ref().map(|pt| pt as *const POINT),
                Some(&SIZE {
                    cx: width,
                    cy: height,
                }),
                Some(memory),
                Some(&POINT { x: 0, y: 0 }),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            window.frame = Some(frame);
            ReleaseDC(None, screen);
            result?;
            keep_moving(window);
            if !color_only || style_changed {
                let was_visible = IsWindowVisible(hwnd).as_bool();
                if !was_visible || style_changed || (!color_only && window.parent.is_some()) {
                    let insert_after = if window.parent.is_some() {
                        HWND_TOP
                    } else {
                        HWND_TOPMOST
                    };
                    let mut flags: SET_WINDOW_POS_FLAGS = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
                    if !was_visible {
                        flags |= SWP_SHOWWINDOW;
                    }
                    if style_changed {
                        flags |= SWP_FRAMECHANGED;
                    }
                    let _ = SetWindowPos(hwnd, Some(insert_after), 0, 0, 0, 0, flags);
                }
            }
            if !color_only {
                match window.overlay.place {
                    Place::Taskbar => {
                        window.stop_hover_timer();
                        super::update_rect(&window.id, None);
                        window.ensure_taskbar_timer();
                    }
                    Place::Free => {
                        window.stop_taskbar_timer();
                        if !window.overlay.locked {
                            let rect_pos = dst_point.or_else(|| {
                                let mut rect = RECT::default();
                                GetWindowRect(hwnd, &mut rect).ok().map(|_| POINT {
                                    x: rect.left,
                                    y: rect.top,
                                })
                            });
                            if let Some(pt) = rect_pos {
                                super::update_rect(
                                    &window.id,
                                    Some((pt.x as f64, pt.y as f64, width as f64, height as f64)),
                                );
                            }
                        } else {
                            super::update_rect(&window.id, None);
                        }
                        window.ensure_hover_timer();
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_read_overlays_as_json() {
        let overlay = Overlay {
            lines: vec![Line {
                text: "你好 \"world\"".into(),
                ruby: String::new(),
                size: 28.0,
                bold: true,
                color: [255, 128, 0, 255],
                lit: Some(2.0),
                route: None,
            }],
            place: Place::Taskbar,
            locked: true,
            buttons: vec![Button {
                id: "lock".into(),
                symbol: "lock.open.fill".into(),
                while_locked: true,
            }],
        };
        assert_eq!(
            as_json(&overlay, TextStyle::default()),
            r##"{"lines":[{"text":"你好 \"world\"","ruby":"","size":28,"bold":true,"color":"#ffff8000","lit":2}],"place":"taskbar","locked":true,"buttons":[{"id":"lock","symbol":"lock.open.fill","whileLocked":true}],"effect":"shadow","opacity":1}"##
        );
    }

    #[test]
    fn what_is_faded_starts_after_the_lit_characters() {
        let overlay = |text: &str, lit| Overlay {
            lines: vec![Line {
                text: text.into(),
                ruby: String::new(),
                size: 10.0,
                bold: false,
                color: [0; 4],
                lit,
                route: None,
            }],
            ..Overlay::default()
        };
        let shown = overlay("a", Some(1.0));
        assert_eq!(shown.change_to(&overlay("a", Some(1.0))), Change::Nothing);
        assert_eq!(shown.change_to(&overlay("a", Some(1.5))), Change::Lit);
        assert_eq!(shown.change_to(&overlay("a", None)), Change::All);
        assert_eq!(shown.change_to(&overlay("b", Some(1.5))), Change::All);
        let mut recolored = shown.clone();
        recolored.lines[0].color = [255, 64, 128, 255];
        assert_eq!(shown.change_to(&recolored), Change::Color);
        recolored.lines[0].lit = Some(1.5);
        assert_eq!(shown.change_to(&recolored), Change::Color);
        recolored.lines[0].size = 12.0;
        assert_eq!(shown.change_to(&recolored), Change::All);
    }

    fn ruby_line(text: &str, ruby: &str) -> Line {
        Line {
            text: text.into(),
            ruby: ruby.into(),
            size: 28.0,
            bold: false,
            color: [255; 4],
            lit: Some(0.0),
            route: None,
        }
    }

    #[test]
    fn ruby_ranges_count_unicode_scalars_and_reject_invalid_spans() {
        let line = ruby_line(
            "a😀漢字",
            r#"[[2,4,"かんじ"],[0,1,"えー"],[1,1,"empty"],[3,5,"outside"],[-1,1,"negative"],[2,3,"overlap"],[1.5,2,"fraction"],[1,2,7]]"#,
        );
        // Sorting is stable by source start/end; overlapping entries lose
        // to the first accepted range, and emoji counts as one scalar.
        assert_eq!(
            line.ruby_spans(),
            vec![(0, 1, "えー".into()), (2, 3, "overlap".into())]
        );
        assert_eq!(
            ruby_line("😀漢字", r#"[[1,3,"かんじ"]]"#).ruby_spans(),
            vec![(1, 3, "かんじ".into())]
        );
        for invalid in ["", "not json", "{}", "[null]", r#"[[0,1,""]]"#] {
            assert!(ruby_line("漢", invalid).ruby_spans().is_empty());
        }
    }

    #[test]
    fn ruby_is_a_json_string_and_layout_changes_are_not_cheap_lit() {
        let shown = Overlay {
            lines: vec![ruby_line("漢😀", r#"[[0,1,"か\"ん\n"]]"#)],
            ..Overlay::default()
        };
        let value: serde_json::Value =
            serde_json::from_str(&as_json(&shown, TextStyle::default())).unwrap();
        assert_eq!(
            value["lines"][0]["ruby"].as_str(),
            Some(shown.lines[0].ruby.as_str())
        );
        let mut next = shown.clone();
        next.lines[0].lit = Some(0.5);
        assert_eq!(shown.change_to(&next), Change::Lit);
        next.lines[0].ruby = r#"[[0,1,"かん"]]"#.into();
        assert_eq!(shown.change_to(&next), Change::All);
        next.lines[0].ruby = shown.lines[0].ruby.clone();
        next.lines[0].text = "字😀".into();
        assert_eq!(shown.change_to(&next), Change::All);
    }

    #[test]
    fn a_route_moves_evenly_and_waits_between_its_points() {
        let route = Route::new("10 0 11 2 12 2 12.5 3", 0.0, 0.0);
        assert_eq!(route.at(9.0), 0.0);
        assert_eq!(route.at(10.5), 1.0);
        assert_eq!(route.at(11.5), 2.0);
        assert_eq!(route.at(12.25), 2.5);
        assert_eq!(route.at(20.0), 3.0);
        assert_eq!(route.path(), "10 0 11 2 12 2 12.5 3");
        assert_eq!(Route::new("", 0.0, 0.0).at(1.0), 0.0);
    }

    #[test]
    fn a_route_runs_only_on_a_running_clock() {
        let standing = Route::new("0 0 100 5", 50.0, 0.0);
        assert!(!standing.moving());
        assert_eq!(standing.place(), 2.5);
        let long_ago = Route::new("0 0 100 5", 0.0, 1.0);
        assert!(long_ago.moving());
        assert_eq!(long_ago.place(), 5.0);
        assert!(!Route::new("", 0.0, 1.0).moving());
    }

    #[test]
    fn a_routes_steps_stop_at_every_whole_character() {
        let route = Route::new("0 0 3 3 4 3", 0.0, 0.0);
        assert_eq!(
            route.steps(0.5),
            vec![(0.5, 0.5), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0), (4.0, 3.0)]
        );
    }

    #[test]
    fn a_line_on_a_route_lights_up_and_changes_cheaply() {
        let overlay = |clock| Overlay {
            lines: vec![Line {
                text: "ab".into(),
                ruby: String::new(),
                size: 10.0,
                bold: false,
                color: [0; 4],
                lit: None,
                route: Some(Route::new("0 0 1 2", clock, 0.0)),
            }],
            ..Overlay::default()
        };
        assert!(overlay(0.0).lines[0].lights());
        assert_eq!(overlay(0.5).lines[0].lit_now(), Some(1.0));
        assert_eq!(overlay(0.0).change_to(&overlay(0.5)), Change::Lit);
        assert!(
            as_json(&overlay(0.5), TextStyle::default())
                .contains(r#""lit":0,"path":"0 0 1 2","clock":0.5,"since":0"#)
        );
    }

    #[test]
    fn taskbar_room_avoids_widgets_on_left_aligned_taskbar() {
        let bar = Room {
            left: 0,
            right: 1920,
            top: 1032,
            bottom: 1080,
        };
        // Left-aligned taskbar: Start/apps on the left, Widgets next to the
        // notification area on the right.
        let occupied = [(0, 620), (1480, 1650), (1652, 1920)];
        assert_eq!(
            pick_taskbar_room(bar, &occupied, 8, 40, 260, None),
            Some(Room {
                left: 628,
                right: 1472,
                top: 1032,
                bottom: 1080,
            })
        );
    }

    #[test]
    fn taskbar_room_picks_wider_left_gap_and_stays_sticky_for_centered_taskbar() {
        let bar = Room {
            left: 0,
            right: 1920,
            top: 1032,
            bottom: 1080,
        };
        // Centered icons leave a wide left gap and a cramped right gap next to
        // a wide widgets/tray block.
        let occupied = [(0, 180), (700, 1220), (1450, 1920)];
        let left_gap = Room {
            left: 188,
            right: 692,
            top: 1032,
            bottom: 1080,
        };
        let right_gap = Room {
            left: 1228,
            right: 1442,
            top: 1032,
            bottom: 1080,
        };
        // Short text fits on the right beside the notification area.
        assert_eq!(
            pick_taskbar_room(bar, &occupied, 8, 40, 180, None),
            Some(right_gap)
        );
        // Long text moves to the wider left gap instead of truncating on the right.
        assert_eq!(
            pick_taskbar_room(bar, &occupied, 8, 40, 320, None),
            Some(left_gap)
        );
        // Once on the left gap, it stays there while within sticky hysteresis.
        assert_eq!(
            pick_taskbar_room(bar, &occupied, 8, 40, 200, Some(left_gap)),
            Some(left_gap)
        );
    }

    #[test]
    fn taskbar_room_returns_none_when_taskbar_is_full() {
        let bar = Room {
            left: 0,
            right: 1280,
            top: 976,
            bottom: 1024,
        };
        let occupied = [(0, 900), (920, 1280)];
        assert_eq!(pick_taskbar_room(bar, &occupied, 8, 40, 200, None), None);
    }

    #[test]
    fn update_rect_notifies_sync_hook_only_on_deactivation() {
        let id = "test_overlay_sync_deactivate";
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = events.clone();
        set_sync_hook(move |overlay_id| {
            seen.lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(overlay_id.to_string());
        });

        update_rect(id, Some((100.0, 200.0, 300.0, 60.0)));
        assert_eq!(screen_rect(id), Some((100.0, 200.0, 300.0, 60.0)));
        // Resizing or moving an active overlay updates the cached rect for
        // future openings without pushing an already-open utility window.
        update_rect(id, Some((120.0, 210.0, 520.0, 90.0)));
        assert_eq!(screen_rect(id), Some((120.0, 210.0, 520.0, 90.0)));
        assert!(events.lock().unwrap().is_empty());

        // Hiding, locking, or moving to the taskbar clears the rect and fires
        // the sync hook once so anchored utility windows close.
        update_rect(id, None);
        assert_eq!(screen_rect(id), None);
        assert_eq!(*events.lock().unwrap(), vec![id.to_string()]);

        // Repeated None updates do not fire again.
        update_rect(id, None);
        assert_eq!(*events.lock().unwrap(), vec![id.to_string()]);
    }

    #[test]
    fn coalesce_win_ops_collapses_high_rate_color_and_lit_updates() {
        let make = |color: [u8; 4], lit: Option<f64>| Overlay {
            lines: vec![Line {
                text: "hello".into(),
                ruby: String::new(),
                size: 28.0,
                bold: true,
                color,
                lit,
                route: None,
            }],
            place: Place::Free,
            ..Overlay::default()
        };
        let style = TextStyle::default();
        let c1 = make([255, 0, 0, 255], Some(1.0));
        let c2 = make([0, 255, 0, 255], Some(1.5));
        let c3 = make([0, 255, 0, 255], Some(2.0));

        // Consecutive Color and Lit updates collapse into a single latest Color
        // without dropping the pending color change.
        let out = coalesce_win_ops(vec![
            WinOp::Color("lyrics".into(), c1.clone(), style),
            WinOp::Color("lyrics".into(), c2.clone(), style),
            WinOp::Lit("lyrics".into(), c3.clone(), style),
        ]);
        assert_eq!(out, vec![WinOp::Color("lyrics".into(), c3.clone(), style)]);

        // A pending Show absorbs later Color updates as Show so creation/layout runs.
        let out = coalesce_win_ops(vec![
            WinOp::Show("lyrics".into(), c1.clone(), style),
            WinOp::Color("lyrics".into(), c2.clone(), style),
        ]);
        assert_eq!(out, vec![WinOp::Show("lyrics".into(), c2.clone(), style)]);

        // Hide supersedes earlier pending frames, while a subsequent Show still runs.
        let out = coalesce_win_ops(vec![
            WinOp::Show("lyrics".into(), c1.clone(), style),
            WinOp::Color("lyrics".into(), c2.clone(), style),
            WinOp::Hide("lyrics".into()),
            WinOp::Color("lyrics".into(), c3.clone(), style),
        ]);
        assert_eq!(
            out,
            vec![
                WinOp::Hide("lyrics".into()),
                WinOp::Color("lyrics".into(), c3, style),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires an interactive Windows desktop session"]
    fn windows_free_overlay_stays_topmost_and_hover_works_during_color_frames() {
        use std::thread;
        use std::time::{Duration, Instant};
        use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetThreadDpiAwarenessContext,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GWL_EXSTYLE, GetClassNameW, GetCursorPos, GetWindowLongPtrW,
            GetWindowRect, GetWindowThreadProcessId, HTCLIENT, IsWindowVisible, SWP_NOACTIVATE,
            SWP_NOSIZE, SWP_NOZORDER, SendMessageW, SetWindowPos, WM_ENTERSIZEMOVE,
            WM_EXITSIZEMOVE, WM_NCHITTEST, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        };
        use windows::core::BOOL;

        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }

        unsafe extern "system" fn collect_own_overlays(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let out = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
            if pid == std::process::id() {
                let mut buf = [0u16; 64];
                let len = unsafe { GetClassNameW(hwnd, &mut buf) };
                if len > 0 && String::from_utf16_lossy(&buf[..len as usize]) == "BernOverlay" {
                    out.push(hwnd);
                }
            }
            BOOL(1)
        }

        fn find_own_overlay() -> Option<HWND> {
            let mut found = Vec::new();
            unsafe {
                let _ = EnumWindows(
                    Some(collect_own_overlays),
                    LPARAM(&mut found as *mut Vec<HWND> as isize),
                );
            }
            found
                .into_iter()
                .find(|&h| unsafe { IsWindowVisible(h) }.as_bool())
        }

        let make = |rgb: [u8; 4], locked: bool, place: Place| Overlay {
            lines: vec![
                Line {
                    text: "Bern Overlay Probe Line".into(),
                    ruby: String::new(),
                    size: 28.0,
                    bold: true,
                    color: rgb,
                    lit: None,
                    route: None,
                },
                Line {
                    text: "Second Line".into(),
                    ruby: String::new(),
                    size: 18.0,
                    bold: false,
                    color: [220, 220, 220, 215],
                    lit: None,
                    route: None,
                },
            ],
            place,
            locked,
            buttons: vec![
                Button {
                    id: "previous".into(),
                    symbol: "backward.fill".into(),
                    while_locked: false,
                },
                Button {
                    id: "lock".into(),
                    symbol: "lock.fill".into(),
                    while_locked: true,
                },
                Button {
                    id: "next".into(),
                    symbol: "forward.fill".into(),
                    while_locked: false,
                },
            ],
        };

        let mut cursor = POINT::default();
        assert!(unsafe { GetCursorPos(&mut cursor) }.is_ok());

        let id = "bern_win_overlay_probe_live";
        show_styled(
            id,
            &make([255, 255, 255, 255], false, Place::Free),
            TextStyle::default(),
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        let hwnd = loop {
            if let Some(h) = find_own_overlay() {
                break h;
            }
            assert!(
                Instant::now() < deadline,
                "BernOverlay window never became visible"
            );
            thread::sleep(Duration::from_millis(20));
        };
        let ex = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
        assert_ne!(
            ex & WS_EX_TOPMOST.0,
            0,
            "free overlay must have WS_EX_TOPMOST on creation, got Ex=0x{ex:08X}"
        );

        // Move the overlay directly under the current cursor (without moving
        // the user's mouse) and commit its position via WM_EXITSIZEMOVE.
        let mut rect = RECT::default();
        assert!(unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok());
        let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f32 / 96.0;
        let width = rect.right - rect.left;
        let btn_center_y = ((12.0 + 14.0) * scale).round() as i32;
        let place_x = cursor.x - width / 2;
        let place_y = cursor.y - btn_center_y;
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                place_x,
                place_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = SendMessageW(hwnd, WM_EXITSIZEMOVE, Some(WPARAM(0)), Some(LPARAM(0)));
        }

        // Drive 24 ms color updates (simulating high-rate animated text colors)
        // for ~530 ms and verify hover stays responsive and buttons are hit-testable.
        let mut saw_button_hit = false;
        for step in 0..22u8 {
            let color = [255 - step * 8, 60 + step * 8, 120 + step * 5, 255];
            show_styled(id, &make(color, false, Place::Free), TextStyle::default());
            thread::sleep(Duration::from_millis(24));
            if let Some(h) = find_own_overlay() {
                let ex_now = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) } as u32;
                assert_ne!(
                    ex_now & WS_EX_TOPMOST.0,
                    0,
                    "lost WS_EX_TOPMOST during color frame {step}: Ex=0x{ex_now:08X}"
                );
                let mut r = RECT::default();
                if unsafe { GetWindowRect(h, &mut r) }.is_ok() {
                    let bx = (r.left + r.right) / 2;
                    let by = r.top + btn_center_y;
                    let lparam = LPARAM((((by & 0xffff) << 16) | (bx & 0xffff)) as isize);
                    let hit =
                        unsafe { SendMessageW(h, WM_NCHITTEST, Some(WPARAM(0)), Some(lparam)) };
                    if hit.0 == HTCLIENT as isize {
                        saw_button_hit = true;
                    }
                }
            }
        }
        assert!(
            saw_button_hit,
            "HOVER_TIMER was starved during 24ms color updates: button bar never became interactive"
        );

        // Simulated drag during 24 ms color frames must not snap the window back.
        let drag_x = place_x + 40;
        let drag_y = place_y + 20;
        unsafe {
            let _ = SendMessageW(hwnd, WM_ENTERSIZEMOVE, Some(WPARAM(0)), Some(LPARAM(0)));
            let _ = SetWindowPos(
                hwnd,
                None,
                drag_x,
                drag_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        for step in 0..5u8 {
            let color = [100 + step * 20, 200, 255 - step * 20, 255];
            show_styled(id, &make(color, false, Place::Free), TextStyle::default());
            thread::sleep(Duration::from_millis(24));
        }
        let mut dragged = RECT::default();
        assert!(unsafe { GetWindowRect(hwnd, &mut dragged) }.is_ok());
        assert_eq!(
            (dragged.left, dragged.top),
            (drag_x, drag_y),
            "color frames snapped dragging window back to old position"
        );
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                place_x,
                place_y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = SendMessageW(hwnd, WM_EXITSIZEMOVE, Some(WPARAM(0)), Some(LPARAM(0)));
        }

        // Locked + cursor over unlock button during 24 ms color frames must clear
        // WS_EX_TRANSPARENT while keeping WS_EX_TOPMOST.
        show_styled(
            id,
            &make([255, 255, 255, 255], true, Place::Free),
            TextStyle::default(),
        );
        for step in 0..5u8 {
            let color = [255, 80 + step * 30, 80, 255];
            show_styled(id, &make(color, true, Place::Free), TextStyle::default());
            thread::sleep(Duration::from_millis(24));
        }
        if let Some(h) = find_own_overlay() {
            let ex_locked = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) } as u32;
            assert_ne!(
                ex_locked & WS_EX_TOPMOST.0,
                0,
                "lost WS_EX_TOPMOST while locked"
            );
            assert_eq!(
                ex_locked & WS_EX_TRANSPARENT.0,
                0,
                "unlock button under cursor must remain clickable while locked during color frames"
            );
        }

        // Switch Taskbar -> Free and verify recreated HWND restores WS_EX_TOPMOST.
        show_styled(
            id,
            &make([255, 255, 255, 255], false, Place::Taskbar),
            TextStyle::default(),
        );
        thread::sleep(Duration::from_millis(120));
        show_styled(
            id,
            &make([255, 255, 255, 255], false, Place::Free),
            TextStyle::default(),
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        let h_after = loop {
            if let Some(h) = find_own_overlay() {
                break h;
            }
            assert!(Instant::now() < deadline, "overlay did not return to Free");
            thread::sleep(Duration::from_millis(20));
        };
        let ex_after = unsafe { GetWindowLongPtrW(h_after, GWL_EXSTYLE) } as u32;
        assert_ne!(
            ex_after & WS_EX_TOPMOST.0,
            0,
            "lost WS_EX_TOPMOST after Taskbar -> Free transition: Ex=0x{ex_after:08X}"
        );

        hide(id);
        thread::sleep(Duration::from_millis(50));
    }
}
