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
    /// Only how far the lines are lit changed since `show`.
    fn lit(&self, id: &str, overlay: &Overlay) {
        self.show(id, overlay);
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
static SHOWN: Mutex<BTreeMap<String, Overlay>> = Mutex::new(BTreeMap::new());

/// What an overlay needs for what it shows next.
#[derive(Debug, PartialEq, Eq)]
enum Change {
    Nothing,
    /// Only how far a line is lit, or the route it follows.
    Lit,
    All,
}

impl Overlay {
    fn change_to(&self, next: &Overlay) -> Change {
        if self == next {
            return Change::Nothing;
        }
        // Text and ruby are layout, not timing: changing either must rebuild
        // the native surface even when only the clock also moved.
        let same = |line: &Line, other: &Line| {
            Line {
                lit: None,
                route: None,
                ..line.clone()
            } == Line {
                lit: None,
                route: None,
                ..other.clone()
            } && line.lit.is_some() == other.lit.is_some()
                && line.route.is_some() == other.route.is_some()
        };
        let lines = self.lines.len() == next.lines.len()
            && self.lines.iter().zip(&next.lines).all(|(a, b)| same(a, b));
        let rest = Overlay {
            lines: Vec::new(),
            ..self.clone()
        } == Overlay {
            lines: Vec::new(),
            ..next.clone()
        };
        if lines && rest {
            Change::Lit
        } else {
            Change::All
        }
    }
}

/// Show `overlay` as the overlay `id`, putting it up when it is not.
pub fn show(id: &str, overlay: &Overlay) {
    let was = SHOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_string(), overlay.clone());
    let change = was.map_or(Change::All, |was| was.change_to(overlay));
    match (change, host()) {
        (Change::Nothing, _) => {}
        (Change::Lit, Some(host)) => host.lit(id, overlay),
        (Change::All, Some(host)) => host.show(id, overlay),
        (Change::Lit, None) => lit_here(id, overlay),
        (Change::All, None) => show_here(id, overlay),
    }
}

/// Take the overlay `id` down.
pub fn hide(id: &str) {
    SHOWN.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
    match host() {
        Some(host) => host.hide(id),
        None => hide_here(id),
    }
}

/// Change how far an overlay of this system's own windows is lit.
fn lit_here(id: &str, overlay: &Overlay) {
    #[cfg(target_os = "macos")]
    mac::lit(id.to_string(), overlay.clone());
    #[cfg(target_os = "windows")]
    win::lit(id.to_string(), overlay.clone());
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_lit", id, &as_json(overlay));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, overlay);
}

/// Show an overlay with this system's own windows.
fn show_here(id: &str, overlay: &Overlay) {
    #[cfg(target_os = "macos")]
    mac::show(id.to_string(), overlay.clone());
    #[cfg(target_os = "windows")]
    win::show(id.to_string(), overlay.clone());
    #[cfg(target_os = "android")]
    let _ = crate::platform::request("overlay_show", id, &as_json(overlay));
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, overlay);
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

/// An overlay as JSON for a host: its lines, place, lock and buttons.
#[cfg(any(target_os = "android", test))]
fn as_json(overlay: &Overlay) -> String {
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
    format!(
        "{{\"lines\":[{}],\"place\":\"{}\",\"locked\":{},\"buttons\":[{}]}}",
        lines.join(","),
        match overlay.place {
            Place::Free => "free",
            Place::Taskbar => "taskbar",
        },
        overlay.locked,
        buttons.join(",")
    )
}

/// macOS: a borderless, non-activating panel above the other windows, on
/// every space and over full-screen apps, with a label per line. While the
/// pointer is over it, which a poll of the pointer tells, the bar shows
/// and, unlocked, the plate; locked, the panel takes the pointer only over
/// the bar's buttons.
#[cfg(target_os = "macos")]
mod mac {
    use super::{Line, Overlay, Route};
    use dispatch2::{DispatchQueue, DispatchTime};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, NSObject, ProtocolObject};
    use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
    use objc2_app_kit::{
        NSBackingStoreType, NSBox, NSBoxType, NSButton, NSColor, NSEvent, NSFont,
        NSFontAttributeName, NSFontWeightSemibold, NSForegroundColorAttributeName, NSImage,
        NSImageSymbolConfiguration, NSPanel, NSScreen, NSShadow, NSShadowAttributeName,
        NSTextField, NSTitlePosition, NSUserInterfaceItemIdentification, NSView,
        NSWindowCollectionBehavior, NSWindowStyleMask,
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
        let field = styled(line, &line.text, 1.0, mtm);
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
        panel: Retained<NSPanel>,
        plate: Option<Retained<NSBox>>,
        /// Each button's round, and the button.
        buttons: Vec<(Retained<NSBox>, Retained<NSButton>)>,
        locked: bool,
        /// What is lit of each line that shows, for those that light up.
        lit: Vec<Option<Lit>>,
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
                    styled(line, &line.text[..at], 1.0, mtm).frame().size.width
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

    pub fn show(id: String, overlay: Overlay) {
        DispatchQueue::main().exec_async(move || {
            if let Some(mtm) = MainThreadMarker::new() {
                put(mtm, &id, &overlay);
            }
        });
    }

    /// Only how far the lines are lit changed: the panel stays as it is.
    pub fn lit(id: String, overlay: Overlay) {
        DispatchQueue::main().exec_async(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let done = PANELS.with(|panels| {
                let mut panels = panels.borrow_mut();
                let Some(shown) = panels.get_mut(&id) else {
                    return false;
                };
                let lines = overlay.lines.iter().filter(|line| !line.text.is_empty());
                for (lit, line) in shown.lit.iter_mut().zip(lines) {
                    if let Some(lit) = lit {
                        lit.line.lit = line.lit;
                        lit.line.route = line.route.clone();
                        lit.show(mtm);
                    }
                }
                true
            });
            if !done {
                put(mtm, &id, &overlay);
            }
        });
    }

    pub fn hide(id: String) {
        DispatchQueue::main().exec_async(move || {
            PANELS.with(|panels| {
                if let Some(shown) = panels.borrow().get(&id) {
                    keep_place(&id, &shown.panel);
                    shown.panel.orderOut(None);
                }
            });
        });
    }

    /// Remember where the panel is: the middle of its bottom edge.
    fn keep_place(id: &str, panel: &NSPanel) {
        let frame = panel.frame();
        super::save_place(id, frame.origin.x + frame.size.width / 2.0, frame.origin.y);
    }

    /// A line: faded, and over it what is lit, cut off inside the character
    /// `Line::lit` stops in.
    fn label(line: &Line, mtm: MainThreadMarker) -> (Retained<NSView>, Option<Lit>) {
        if !line.lights() {
            return (annotated(line, 1.0, mtm), None);
        }
        let label = annotated(line, super::FADED, mtm);
        let size = label.frame().size;
        let clip = NSView::initWithFrame(
            NSView::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), size),
        );
        clip.setClipsToBounds(true);
        // The text, with its blurred shadow, is drawn once: moving the cut
        // only changes the bounds of a layer.
        clip.setWantsLayer(true);
        let bright = annotated(line, 1.0, mtm);
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
        (label, Some(lit))
    }

    /// Keep the original line as one shaped, accessible text field. Readings
    /// occupy a reserved band above their measured source ranges; they do not
    /// contribute characters to the highlighting clock.
    fn annotated(line: &Line, alpha: f64, mtm: MainThreadMarker) -> Retained<NSView> {
        let base = styled(line, &line.text, alpha, mtm);
        let spans = line.ruby_spans();
        if spans.is_empty() {
            return base.into_super().into_super();
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
            let measured = styled(&style, &reading, alpha, mtm).frame().size.width;
            style.size *= (width / measured.max(1.0)).min(1.0);
            let ruby = styled(&style, &reading, alpha, mtm);
            let ruby_size = ruby.frame().size;
            ruby.setFrameOrigin(NSPoint::new(
                left + (width - ruby_size.width) / 2.0,
                size.height,
            ));
            // The original field remains the accessible text, without a
            // second traversal through the pronunciation decorations.
            let _: () = unsafe { msg_send![&*ruby, setAccessibilityElement: false] };
            view.addSubview(&ruby);
        }
        view
    }

    /// `text` in the style of `line`, its alpha scaled by `alpha`.
    fn styled(line: &Line, text: &str, alpha: f64, mtm: MainThreadMarker) -> Retained<NSTextField> {
        let attributes = NSMutableDictionary::<NSString, AnyObject>::new();
        let font = if line.bold {
            NSFont::boldSystemFontOfSize(line.size)
        } else {
            NSFont::systemFontOfSize(line.size)
        };
        let [r, g, b, a] = line.color.map(|c| c as f64 / 255.0);
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a * alpha);
        let shadow = NSShadow::new();
        shadow.setShadowOffset(NSSize::new(0.0, -1.0));
        shadow.setShadowBlurRadius(3.0);
        shadow.setShadowColor(Some(&NSColor::colorWithWhite_alpha(0.0, 0.85)));
        // SAFETY: the keys are AppKit's attribute names, with values of their
        // kinds.
        unsafe {
            attributes.setObject_forKey(&font, ProtocolObject::from_ref(NSFontAttributeName));
            attributes.setObject_forKey(
                &color,
                ProtocolObject::from_ref(NSForegroundColorAttributeName),
            );
            attributes.setObject_forKey(&shadow, ProtocolObject::from_ref(NSShadowAttributeName));
        }
        // SAFETY: string attributes as AppKit takes them.
        let text = unsafe {
            NSMutableAttributedString::initWithString_attributes(
                NSMutableAttributedString::alloc(),
                &NSString::from_str(text),
                Some(&attributes),
            )
        };
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

    fn put(mtm: MainThreadMarker, id: &str, overlay: &Overlay) {
        let panel = PANELS
            .with(|panels| panels.borrow().get(id).map(|shown| shown.panel.clone()))
            .unwrap_or_else(|| {
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
            });
        let shown = panel.isVisible();
        // Where it was: the middle of its bottom edge, kept as the text
        // changes width.
        let place = if shown {
            let frame = panel.frame();
            Some((frame.origin.x + frame.size.width / 2.0, frame.origin.y))
        } else {
            super::saved_place(id)
        };
        let (labels, lit): (Vec<Retained<NSView>>, Vec<Option<Lit>>) = overlay
            .lines
            .iter()
            .filter(|line| !line.text.is_empty())
            .map(|line| label(line, mtm))
            .unzip();
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
        let shown = Shown {
            panel: panel.clone(),
            plate,
            buttons,
            locked: overlay.locked,
            lit,
        };
        hover(&shown);
        PANELS.with(|panels| panels.borrow_mut().insert(id.to_string(), shown));
        if overlay.lines.iter().all(|line| line.text.is_empty()) {
            panel.orderOut(None);
        } else {
            panel.orderFrontRegardless();
            poll();
        }
        keep_place(id, &panel);
    }

    /// Show the bar and the plate while the pointer is over the panel;
    /// locked, take the pointer only over a button.
    fn hover(shown: &Shown) {
        let pointer = NSEvent::mouseLocation();
        let frame = shown.panel.frame();
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
#[cfg(target_os = "windows")]
mod win {
    use super::{Line, Overlay, Place, Route};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::{Mutex, mpsc};
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
        CUIAutomation, IUIAutomation, TreeScope_Descendants, UIA_BoundingRectanglePropertyId,
        UIA_ButtonControlTypeId, UIA_CONTROLTYPE_ID, UIA_ComboBoxControlTypeId,
        UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_IsOffscreenPropertyId,
        UIA_ListItemControlTypeId, UIA_MenuItemControlTypeId, UIA_SplitButtonControlTypeId,
    };
    use windows::Win32::UI::HiDpi::{
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetThreadDpiAwarenessContext,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowExW, FindWindowW, GWL_EXSTYLE,
        GetCursorPos, GetMessageW, GetWindowLongPtrW, GetWindowRect, HTCAPTION, HTCLIENT,
        HTTRANSPARENT, HWND_TOPMOST, IDC_ARROW, KillTimer, LoadCursorW, MA_NOACTIVATE, MSG,
        PostThreadMessageW, RegisterClassExW, SPI_GETWORKAREA, SW_HIDE, SW_SHOWNOACTIVATE,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetTimer, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, SystemParametersInfoW, TranslateMessage, ULW_ALPHA, UPDATELAYEREDWINDOWINFO,
        UpdateLayeredWindow, UpdateLayeredWindowIndirect, WM_APP, WM_EXITSIZEMOVE, WM_LBUTTONDOWN,
        WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    };
    use windows::core::{BOOL, PCWSTR, w};
    use windows_numerics::Vector2;

    enum Message {
        Show(String, Overlay),
        /// Only how far the lines are lit, or their routes, changed.
        Lit(String, Overlay),
        Hide(String),
    }

    static CHANNEL: Mutex<Option<(mpsc::Sender<Message>, u32)>> = Mutex::new(None);

    /// Around the text and between lines, in points.
    const MARGIN: f32 = 12.0;
    const GAP: f32 = 2.0;
    /// A button of the bar, the room between them, and between the bar and
    /// the lines, in points.
    const BUTTON: f32 = 28.0;
    const BUTTON_GAP: f32 = 8.0;
    const BAR_GAP: f32 = 8.0;
    /// How often an overlay on the taskbar is put back on top of it, and
    /// looks again for the room the taskbar leaves it.
    const TOPMOST_TIMER: usize = 1;
    /// Between an overlay on the taskbar and what is beside it, in pixels.
    const TASKBAR_GAP: i32 = 8;
    /// What takes room on the taskbar, read through UI Automation: its
    /// buttons, the items of its lists, and the search box. Their panes and
    /// lists are left out, as they may stretch over free room.
    const OCCUPYING: [UIA_CONTROLTYPE_ID; 6] = [
        UIA_ButtonControlTypeId,
        UIA_SplitButtonControlTypeId,
        UIA_MenuItemControlTypeId,
        UIA_ListItemControlTypeId,
        UIA_EditControlTypeId,
        UIA_ComboBoxControlTypeId,
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
        ("lock.fill", 0xE72E),
        ("lock.open.fill", 0xE785),
        ("pause.fill", 0xF8AE),
        ("play.fill", 0xF5B0),
        ("xmark", 0xE711),
    ];

    pub fn show(id: String, overlay: Overlay) {
        send(Message::Show(id, overlay));
    }

    pub fn lit(id: String, overlay: Overlay) {
        send(Message::Lit(id, overlay));
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
            let Ok(thread) = started.recv() else { return };
            *channel = Some((send, thread));
        }
        if let Some((sender, thread)) = channel.as_ref()
            && sender.send(message).is_ok()
        {
            // SAFETY: wakes the overlay thread's message loop.
            let _ = unsafe { PostThreadMessageW(*thread, WM_APP, WPARAM(0), LPARAM(0)) };
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
        /// On the taskbar, the room it had when it was last drawn.
        room: Option<Room>,
    }

    /// The free stretch of the taskbar an overlay stands in, in pixels.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Room {
        left: i32,
        right: i32,
        top: i32,
        bottom: i32,
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
    unsafe fn keep_moving(window: &Window) {
        let moving = window
            .overlay
            .lines
            .iter()
            .any(|line| line.route.as_ref().is_some_and(Route::moving));
        // SAFETY: a window of this thread.
        unsafe {
            if moving {
                SetTimer(Some(window.hwnd), LIT_TIMER, LIT_EVERY, None);
            } else {
                let _ = KillTimer(Some(window.hwnd), LIT_TIMER);
            }
        }
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
    }

    thread_local! {
        static WINDOWS: RefCell<HashMap<String, Window>> = RefCell::new(HashMap::new());
        static FACTORIES: RefCell<Option<(ID2D1Factory, IDWriteFactory)>> = const { RefCell::new(None) };
        /// Made the first time the taskbar is read.
        static AUTOMATION: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
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
            WM_EXITSIZEMOVE => {
                let mut rect = RECT::default();
                // SAFETY: the window this procedure serves.
                if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
                    let place = ((rect.left + rect.right) as f64 / 2.0, rect.bottom as f64);
                    with_window(hwnd, |window, _, _| {
                        window.place = Some(place);
                        super::save_place(&window.id, place.0, place.1);
                    });
                }
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
                        let _ = unsafe { draw(window, d2d, dwrite) };
                    }
                });
                LRESULT(0)
            }
            WM_TIMER => {
                // What is on the taskbar changes as windows open and close:
                // draw again in the room it leaves now.
                with_window(hwnd, |window, d2d, dwrite| {
                    // SAFETY: reads the shell's windows, and draws a window of
                    // this thread.
                    unsafe {
                        if window.overlay.place == Place::Taskbar && taskbar_room() != window.room {
                            let _ = draw(window, d2d, dwrite);
                        }
                    }
                });
                // SAFETY: keeps a window of this thread on top of the taskbar.
                let _ = unsafe {
                    SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                };
                LRESULT(0)
            }
            // SAFETY: the default handling of a window of this thread.
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn run(messages: mpsc::Receiver<Message>, ready: mpsc::Sender<u32>) {
        // SAFETY: this thread's own settings, window class and factories.
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            // UI Automation reads the taskbar. In the multithreaded apartment
            // its calls wait without pumping this thread's messages.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let instance = GetModuleHandleW(None).unwrap_or_default();
            let class = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                lpszClassName: w!("BernOverlay"),
                ..Default::default()
            };
            RegisterClassExW(&class);
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
            let _ = ready.send(GetCurrentThreadId());
            let mut message = MSG::default();
            while GetMessageW(&mut message, None, 0, 0).as_bool() {
                if message.hwnd.is_invalid() && message.message == WM_APP {
                    while let Ok(next) = messages.try_recv() {
                        FACTORIES.with(|factories| {
                            if let Some((d2d, dwrite)) = factories.borrow().as_ref() {
                                apply(next, d2d, dwrite, instance.into());
                            }
                        });
                    }
                    continue;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe fn apply(
        message: Message,
        d2d: &ID2D1Factory,
        dwrite: &IDWriteFactory,
        instance: windows::Win32::Foundation::HINSTANCE,
    ) {
        match message {
            Message::Hide(id) => WINDOWS.with(|windows| {
                if let Some(window) = windows.borrow_mut().get_mut(&id) {
                    window.over = false;
                    window.hot = None;
                    // SAFETY: a window of this thread.
                    unsafe {
                        let _ = KillTimer(Some(window.hwnd), TOPMOST_TIMER);
                        let _ = KillTimer(Some(window.hwnd), HOVER_TIMER);
                        let _ = KillTimer(Some(window.hwnd), LIT_TIMER);
                        let _ = ShowWindow(window.hwnd, SW_HIDE);
                    }
                }
            }),
            Message::Lit(id, overlay) => {
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
                    unsafe { apply(Message::Show(id, overlay), d2d, dwrite, instance) };
                }
            }
            Message::Show(id, overlay) => WINDOWS.with(|windows| {
                let mut windows = windows.borrow_mut();
                let window = windows.entry(id.clone()).or_insert_with(|| {
                    // SAFETY: a window of the class this thread registered.
                    let hwnd = unsafe {
                        CreateWindowExW(
                            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                            w!("BernOverlay"),
                            w!(""),
                            WS_POPUP,
                            0,
                            0,
                            1,
                            1,
                            None,
                            None,
                            Some(instance),
                            None,
                        )
                    }
                    .unwrap_or_default();
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
                        room: None,
                    }
                });
                window.overlay = overlay;
                // SAFETY: draws a window of this thread.
                let _ = unsafe { draw(window, d2d, dwrite) };
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
            let mut fonts: Option<IDWriteFontCollection> = None;
            dwrite.GetSystemFontCollection(&mut fonts, false).ok()?;
            let (mut index, mut exists) = (0u32, BOOL(0));
            let fluent = fonts
                .and_then(|fonts| {
                    fonts
                        .FindFamilyName(w!("Segoe Fluent Icons"), &mut index, &mut exists)
                        .ok()
                })
                .is_some()
                && exists.as_bool();
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

    /// The room an overlay on the taskbar has: from the last thing on the
    /// taskbar to the notification area, the taskbar's height, in pixels.
    /// What is on it is read through UI Automation, as the taskbar of
    /// Windows 11 draws its buttons, widgets and search box in XAML, with
    /// no windows of their own.
    unsafe fn taskbar_room() -> Option<Room> {
        // SAFETY: reads the shell's windows.
        unsafe {
            let tray = FindWindowW(w!("Shell_TrayWnd"), None).ok()?;
            let mut bar = RECT::default();
            GetWindowRect(tray, &mut bar).ok()?;
            let mut right = bar.right;
            if let Ok(notify) = FindWindowExW(Some(tray), None, w!("TrayNotifyWnd"), None) {
                let mut area = RECT::default();
                if GetWindowRect(notify, &mut area).is_ok() && area.left > bar.left {
                    right = area.left;
                }
            }
            let mut left = bar.left;
            // A taskbar on the side of the screen keeps its whole width.
            if bar.right - bar.left > bar.bottom - bar.top {
                for item in taskbar_items(tray, &bar) {
                    if item.right <= right {
                        left = left.max(item.right);
                    }
                }
            }
            Some(Room {
                left: left + TASKBAR_GAP,
                right: right - TASKBAR_GAP,
                top: bar.top,
                bottom: bar.bottom,
            })
        }
    }

    /// Where each thing that takes room on the taskbar is, in pixels.
    unsafe fn taskbar_items(tray: HWND, bar: &RECT) -> Vec<RECT> {
        AUTOMATION.with(|automation| {
            let mut automation = automation.borrow_mut();
            if automation.is_none() {
                // SAFETY: COM is initialized on this thread in `run`.
                *automation =
                    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();
            }
            let Some(automation) = automation.as_ref() else {
                return Vec::new();
            };
            // SAFETY: reads the shell's window through UI Automation.
            unsafe { read_taskbar(automation, tray, bar) }.unwrap_or_default()
        })
    }

    unsafe fn read_taskbar(
        automation: &IUIAutomation,
        tray: HWND,
        bar: &RECT,
    ) -> windows::core::Result<Vec<RECT>> {
        // SAFETY: UI Automation calls on objects made on this thread.
        unsafe {
            let root = automation.ElementFromHandle(tray)?;
            let cache = automation.CreateCacheRequest()?;
            for property in [
                UIA_BoundingRectanglePropertyId,
                UIA_ControlTypePropertyId,
                UIA_IsOffscreenPropertyId,
            ] {
                cache.AddProperty(property)?;
            }
            let shown = automation.ControlViewCondition()?;
            let found = root.FindAllBuildCache(TreeScope_Descendants, &shown, &cache)?;
            // Wider than half the taskbar is not one thing on it.
            let half = (bar.right - bar.left) / 2;
            let mut items = Vec::new();
            for index in 0..found.Length()? {
                let Ok(element) = found.GetElement(index) else {
                    continue;
                };
                let (Ok(rect), Ok(kind), Ok(offscreen)) = (
                    element.CachedBoundingRectangle(),
                    element.CachedControlType(),
                    element.CachedIsOffscreen(),
                ) else {
                    continue;
                };
                let width = rect.right - rect.left;
                if offscreen.as_bool()
                    || !OCCUPYING.contains(&kind)
                    || width <= 0
                    || width > half
                    || rect.bottom <= bar.top
                    || rect.top >= bar.bottom
                {
                    continue;
                }
                items.push(rect);
            }
            Ok(items)
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
    ) -> windows::core::Result<()> {
        let hwnd = window.hwnd;
        // SAFETY: Win32, Direct2D and DirectWrite calls on this thread's
        // window and its own device contexts and bitmap.
        unsafe {
            let dpi = GetDpiForWindow(hwnd).max(96);
            let scale = dpi as f32 / 96.0;
            let free = window.overlay.place == Place::Free;
            // On the taskbar the lines fill its height, with no room around.
            let (margin, gap) = if free {
                (MARGIN * scale, GAP * scale)
            } else {
                (2.0 * scale, 0.0)
            };
            let room = if free { None } else { taskbar_room() };
            if room != window.room {
                if let Some(room) = room {
                    eprintln!(
                        "bern overlay: taskbar room {}..{} of {}..{}",
                        room.left, room.right, room.top, room.bottom
                    );
                }
                window.room = room;
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
                let _ = KillTimer(Some(hwnd), HOVER_TIMER);
                let _ = KillTimer(Some(hwnd), LIT_TIMER);
                let _ = ShowWindow(hwnd, SW_HIDE);
                window.frame = None;
                // Still on the taskbar, it looks for room again every second.
                if room.is_some() {
                    SetTimer(Some(hwnd), TOPMOST_TIMER, 1000, None);
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
                let shadow = dark(0.75)?;
                let mut top = margin + bar_height;
                for (layout, line_width, line_height, [r, g, b, a], index, readings, band) in &lines
                {
                    let left = (width as f32 - line_width) / 2.0;
                    let alpha = if dim && index.is_some() {
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
                    // A soft shadow: the text a little down and around, dark,
                    // under the text itself.
                    for (dx, dy) in [(1.0, 1.5), (0.0, 1.0), (1.0, 0.0), (-0.5, 1.0)] {
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
                        for (dx, dy) in [(1.0, 1.5), (0.0, 1.0), (1.0, 0.0), (-0.5, 1.0)] {
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

            let (left, top) = match window.overlay.place {
                Place::Taskbar => room.map_or((0, 0), |room| {
                    (
                        room.right - width,
                        (room.top + room.bottom) / 2 - height / 2,
                    )
                }),
                Place::Free => {
                    let (x, y) = window.place.unwrap_or_else(|| {
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
                    });
                    ((x - width as f64 / 2.0) as i32, (y - height as f64) as i32)
                }
            };
            // Locked, clicks go through but over a button that shows.
            let through = window.overlay.locked && !(window.over && window.hot.is_some());
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
            let style = if through {
                style | WS_EX_TRANSPARENT.0
            } else {
                style & !WS_EX_TRANSPARENT.0
            };
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style as isize);
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let result = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                Some(&POINT { x: left, y: top }),
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
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            match window.overlay.place {
                Place::Taskbar => SetTimer(Some(hwnd), TOPMOST_TIMER, 1000, None),
                Place::Free => SetTimer(Some(hwnd), HOVER_TIMER, HOVER_EVERY, None),
            };
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
            as_json(&overlay),
            r##"{"lines":[{"text":"你好 \"world\"","ruby":"","size":28,"bold":true,"color":"#ffff8000","lit":2}],"place":"taskbar","locked":true,"buttons":[{"id":"lock","symbol":"lock.open.fill","whileLocked":true}]}"##
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
        let value: serde_json::Value = serde_json::from_str(&as_json(&shown)).unwrap();
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
            as_json(&overlay(0.5)).contains(r#""lit":0,"path":"0 0 1 2","clock":0.5,"since":0"#)
        );
    }
}
