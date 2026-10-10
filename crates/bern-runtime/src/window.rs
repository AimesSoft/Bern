//! Presentation of window instances.
//!
//! The target owns the real windows. It reports what the platform shows —
//! minimized, maximized, moved, focused — as facts tagged with their
//! source and the instance generation, and the runtime keeps the latest
//! facts of each instance with a version. Facts never dispatch commands:
//! a window the user minimized is recorded as minimized, not minimized
//! again, and external entries have no way to inject facts.

use bern_protocol::Value;

/// How a window kind is presented by the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    /// A system window, such as an `NSWindow`.
    Native,
    /// A window drawn inside a host surface, such as a page.
    Virtual,
}

/// Where the window is, as the platform reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    Minimized,
    /// Not on screen and not minimized, for example while the app is hidden.
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeState {
    Normal,
    Maximized,
    FullScreen,
}

/// Window actions; `WindowFacts::actions` lists those available now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowAction {
    Activate,
    Close,
    Minimize,
    Restore,
    Maximize,
    Move,
    Resize,
}

impl WindowAction {
    pub const ALL: [WindowAction; 7] = [
        WindowAction::Activate,
        WindowAction::Close,
        WindowAction::Minimize,
        WindowAction::Restore,
        WindowAction::Maximize,
        WindowAction::Move,
        WindowAction::Resize,
    ];

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|action| *action == self).unwrap() as u32
    }

    pub fn name(self) -> &'static str {
        match self {
            WindowAction::Activate => "activate",
            WindowAction::Close => "close",
            WindowAction::Minimize => "minimize",
            WindowAction::Restore => "restore",
            WindowAction::Maximize => "maximize",
            WindowAction::Move => "move",
            WindowAction::Resize => "resize",
        }
    }

    /// Decode a set of actions from bit `index()` of `bits`.
    pub fn from_bits(bits: u32) -> Vec<WindowAction> {
        Self::ALL
            .into_iter()
            .filter(|action| bits & (1 << action.index()) != 0)
            .collect()
    }
}

/// Who caused the latest change of a window's facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactSource {
    /// A window command from any entry.
    Command,
    /// The user or the system, for example the minimize button.
    System,
}

/// Outer frame in points. The origin is the top-left corner of the main
/// display and `y` grows downwards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// What the platform shows for one window instance.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowFacts {
    pub visibility: Visibility,
    pub size: SizeState,
    pub frame: Frame,
    pub focused: bool,
    pub actions: Vec<WindowAction>,
}

/// Lifecycle of an open window instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// Attached, but the target has not reported its presentation yet.
    Opening,
    Open,
    /// A close confirmation is waiting for a decision.
    Closing,
}

/// `window.list` entry for one open instance.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowInfo {
    pub window: String,
    pub kind: String,
    pub presentation: Presentation,
    pub lifecycle: Lifecycle,
    /// `None` while the window is opening.
    pub facts: Option<WindowFacts>,
    /// Bumped by every change of the facts of this instance.
    pub version: u64,
    pub changed_by: Option<FactSource>,
}

impl WindowInfo {
    /// The session catalog's `WindowInfo` record. While the window is
    /// opening it is hidden, has no frame and offers no actions.
    pub fn to_value(&self) -> Value {
        let variant = |index: usize| Value::unit_variant(index as u32);
        let facts = self.facts.as_ref();
        Value::Record(vec![
            Value::Text(self.window.clone()),
            Value::Text(self.kind.clone()),
            variant(self.presentation as usize),
            variant(self.lifecycle as usize),
            variant(facts.map_or(Visibility::Hidden, |facts| facts.visibility) as usize),
            variant(facts.map_or(SizeState::Normal, |facts| facts.size) as usize),
            match facts {
                Some(facts) => Value::some(Value::Record(vec![
                    Value::Float(facts.frame.x),
                    Value::Float(facts.frame.y),
                    Value::Float(facts.frame.width),
                    Value::Float(facts.frame.height),
                ])),
                None => Value::none(),
            },
            Value::Bool(facts.is_some_and(|facts| facts.focused)),
            Value::List(
                facts
                    .map(|facts| facts.actions.as_slice())
                    .unwrap_or_default()
                    .iter()
                    .map(|action| variant(action.index() as usize))
                    .collect(),
            ),
            Value::Int(self.version as i64),
            match self.changed_by {
                Some(source) => Value::some(variant(source as usize)),
                None => Value::none(),
            },
        ])
    }
}
