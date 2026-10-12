//! GTK 4 runtime of the `gtk + linux` target.
//!
//! Generated applications describe themselves with an [`AppDef`] and build
//! their windows from the functions in this crate. The engine, the UI
//! session runtime and this frontend share one process through the same
//! in-process interface the SwiftUI and Web targets use (`bern-ffi`):
//! values cross it in Bern's binary layout, external entries reach the
//! live UI through session requests, and media runs in the frontend's
//! executor.
//!
//! Widgets follow the GNOME platform: GTK 4 with libadwaita's standard
//! containers, rows and styles, colors from the system style manager.

mod controls;
pub mod ctx;
mod host;
mod icons;
mod media;
mod parent;
mod picture;
mod query;
pub mod reactive;
mod session;
mod snapshot;
mod structure;
pub mod text;
pub mod value;
mod widgets;
mod windows;

pub use bern_ffi;
pub use bern_ffi::protocol;
pub use ctx::{Ctx, Field};
pub use glib;
pub use host::{dispatch, run};
pub use media::{MediaOp, Video, VideoProps, media, media_open};
pub use parent::Parent;
pub use query::{Query, QueryProps};
pub use structure::*;
pub use widgets::*;
pub use windows::{close_window, open_window};

use protocol::{ProtocolDescriptor, Type, TypeTable, Value};
use std::rc::Rc;

/// An expression of the declaration, evaluated in a context.
pub type Get = Rc<dyn Fn(&Ctx) -> Value>;
/// A text of the declaration.
pub type Str = Rc<dyn Fn(&Ctx) -> String>;
/// An action bound to a command.
pub type Act = Rc<dyn Fn(&Ctx)>;
/// Builds children into a parent.
pub type Build = Rc<dyn Fn(&Parent, &Ctx)>;
/// Picks the arm of conditional content, or none.
pub type Branch = Rc<dyn Fn(&Ctx) -> Option<usize>>;
/// Runs a declared save command; closing waits for its result.
pub type Save = Rc<dyn Fn(&Ctx, Done)>;

/// Callbacks the runtime keeps.
pub(crate) type Thunk = Rc<dyn Fn()>;
pub(crate) type TextFn = Rc<dyn Fn() -> String>;
pub(crate) type BytesFn = Rc<dyn Fn(&[u8])>;

pub fn get(f: impl Fn(&Ctx) -> Value + 'static) -> Get {
    Rc::new(f)
}

pub fn str(f: impl Fn(&Ctx) -> String + 'static) -> Str {
    Rc::new(f)
}

pub fn act(f: impl Fn(&Ctx) + 'static) -> Act {
    Rc::new(f)
}

pub fn build(f: impl Fn(&Parent, &Ctx) + 'static) -> Build {
    Rc::new(f)
}

/// A failed command, shown to the user and reported to callers.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub code: String,
    pub message: String,
}

impl Failure {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Completion of a command.
pub type Done = Box<dyn FnOnce(Result<(), Failure>)>;

/// Typed drafts of a UI session, one instance per hosting window.
pub struct SessionDef {
    pub name: &'static str,
    /// `(name, type, initial)`.
    pub fields: Vec<(&'static str, Type, Value)>,
}

/// What a window's close check found.
pub struct CloseCheck {
    pub needs_confirmation: bool,
    pub message: String,
    /// Runs the declared save command; closing waits for its result.
    pub save: Option<Save>,
}

/// A declared window kind.
pub struct WindowDef {
    pub id: &'static str,
    /// Text key of the title.
    pub title: &'static str,
    pub settings: bool,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub initial_open: bool,
    /// Global indices of the session types each instance owns.
    pub sessions: Vec<usize>,
    pub render: fn(&Parent, &Ctx),
    pub close: Option<fn(&Ctx) -> CloseCheck>,
}

/// The engine of a generated application, started with the frontend's
/// callbacks and the directory holding the declared resources.
pub type StartEngine = Box<
    dyn FnOnce(bern_ffi::BernCallbacks, &str, *mut bern_ffi::BernBuffer) -> *mut bern_ffi::BernApp,
>;

/// Everything the runtime needs to know about a generated application.
pub struct AppDef {
    pub id: &'static str,
    pub bundle_id: &'static str,
    /// Text key of the application name.
    pub name: &'static str,
    pub version: &'static str,
    pub fingerprint: u64,
    pub languages: &'static [&'static str],
    pub default_language: &'static str,
    pub strings: text::StringTable,
    pub descriptor: ProtocolDescriptor,
    /// Value types of the protocol and the UI declarations.
    pub types: TypeTable,
    pub sessions: Vec<SessionDef>,
    pub windows: Vec<WindowDef>,
    pub primary: &'static str,
    pub players: Vec<&'static str>,
    /// Available width below which `Adaptive` uses its compact layout.
    pub compact_below: Option<f64>,
    /// Brand color, mapped to the accent color.
    pub brand_seed: Option<(u8, u8, u8)>,
}
