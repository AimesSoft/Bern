//! GPUI runtime of the `gpui` target.
//!
//! Generated applications describe themselves with an [`AppDef`]; each
//! window's content is a function the generated code provides, run again
//! whenever the window renders, that reads engine state, drafts and locals
//! from a [`Ctx`]. The engine, the UI session runtime and this frontend
//! share one process through the in-process interface the other native
//! targets use (`bern-ffi`): values cross it in Bern's binary layout.
//!
//! GPUI has no platform controls or colors of its own, so controls are
//! drawn after Bern's own visual specification, which the Web target
//! shares, in colors the Bern theme algorithm derives for the window's
//! light or dark appearance. The behavior of text fields comes from
//! GPUI Kit's unstyled `gpui-base`.

mod behaviors;
mod canvas;
mod content;
mod controls;
mod ctx;
mod form;
mod host;
mod icons;
mod layout;
mod look;
mod markdown;
mod media;
mod menus;
mod native;
mod navigation;
mod overlay;
mod pictures;
mod query;
mod scene;
mod scroll;
mod search;
mod session;
mod spin;
pub use spin::Spin;
pub mod text;
mod theme;
mod units;
pub mod value;
mod windows;

pub use behaviors::{
    ShortcutInputProps, SliderProps, focus_on_appear, focus_region, pinch, shortcut,
    shortcut_input, slider, timer, toast, watch,
};
pub use bern_ffi;
pub use bern_ffi::protocol;
pub use canvas::{
    Band, Fade, Op, PatternOp, SeriesOp, ShapeKind, ShapeOp, canvas_element as canvas,
};
pub use content::{Emphasis, Fit, TextProps, TextStyle, image, image_url, label, progress, text};
pub use controls::{
    ButtonProps, IconButtonProps, InputKind, InputProps, PickerStyle, Prominence, Role,
    SwitchProps, SwitchStyle, button, button_with, drop_area, icon_button, language_picker, picker,
    switch, text_input,
};
pub use ctx::{
    Act, Ctx, Field, Model, act, session_pop, session_push_str, session_set, session_toggle,
};
pub use form::{FormProps, SectionProps, form, section};
pub use gpui;
pub use host::{WindowOp, dispatch, window_command};
pub use layout::{
    AccordionProps, Align, Flex, GridProps, Grow, Layers, Len, Pad, PreviewProps, Size, Style,
    accordion, adaptive, column, divider, flow, grid, preview_list, reveal, row, scroll, spacer,
    stack, tabs,
};
pub use look::{ControlSize, ControlStyle, Look, Shape};
pub use media::{MediaOp, VideoProps, media, video};
pub use menus::{MenuEntry, MenuProps, control_group, menu, menu_item};
pub use navigation::{
    detail, nav_link, navigation, sidebar, sidebar_item, sidebar_title, split_view,
};
pub use overlay::{ModalProps, PopoverProps, SplitProps, modal, popover_button, split_button};
pub use query::{QueryProps, query};
pub use scene::{Anchor as RailAnchor, Place, Placement, positioned, rail, scale_box};
pub use scroll::{ScrollProps, Snap, scroll_items, scroll_key, scroll_view};
pub use search::{SearchItemProps, SearchProps, search, search_item, toolbar};

use bern_ffi::protocol::{ProtocolDescriptor, Type, TypeTable, Value};

/// An element of a window's content.
pub type El = gpui::AnyElement;

/// A command's outcome as the frontend shows it.
#[derive(Clone, Debug)]
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

/// Starts the engine with the frontend's callbacks and the directory of
/// the declared resources; writes a message to the buffer on failure.
pub type StartEngine = Box<
    dyn FnOnce(bern_ffi::BernCallbacks, &str, *mut bern_ffi::BernBuffer) -> *mut bern_ffi::BernApp,
>;

/// Typed drafts of a UI session, one instance per hosting window.
pub struct SessionDef {
    pub name: &'static str,
    /// `(name, type, initial)`.
    pub fields: Vec<(&'static str, Type, Value)>,
}

/// A color: a theme role, or one fixed per appearance.
#[derive(Clone, Copy, Debug)]
pub enum Color {
    Role(&'static str),
    Fixed {
        light: (u8, u8, u8),
        dark: (u8, u8, u8),
    },
}

/// The appearance every window shows (`WindowSet(appearance:)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

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
    /// Available width below which the layout size is `compact`, `medium`
    /// and `expanded`; `large` from the last.
    pub compact_below: Option<f64>,
    pub medium_below: Option<f64>,
    pub expanded_below: Option<f64>,
    /// Brand color, the seed of the theme.
    pub brand_seed: Option<(u8, u8, u8)>,
    /// The accent chosen at run time: its seed in the light and dark
    /// appearance.
    pub accent: Option<Accent>,
    /// The appearance chosen at run time.
    pub appearance: Option<fn(&Ctx) -> Appearance>,
    /// The picture of `[app.icon]` among the resources, when it can be
    /// shown.
    pub app_icon: Option<&'static str>,
    /// The icon the window set chooses at run time (`WindowSet(icon:)`),
    /// as a picture among the resources.
    pub window_icon: Option<fn(&Ctx) -> Option<&'static str>>,
    /// The icons the application shows, as SVG documents by name.
    pub icons: &'static [(&'static str, &'static str)],
    /// Text styles with weights (and desktop sizes) of their own, from
    /// `text { … }` in `style!`.
    pub text_weights: &'static [(&'static str, u16, Option<f32>)],
}

/// A window kind of the `WindowSet`.
pub struct WindowDef {
    pub id: &'static str,
    /// Text key of the title.
    pub title: &'static str,
    /// Opened when the application starts.
    pub open: bool,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    /// Content under a transparent title bar (`chrome: unified`).
    pub unified: bool,
    /// Seconds without input after which `window.idle` holds.
    pub idle_after: Option<f64>,
    /// While true, the toolbar hides and the pointer hides once the window
    /// is idle (`immersive`).
    pub immersive: Option<fn(&Ctx) -> bool>,
    /// While false, the window is never idle (`idle_enabled`).
    pub idle_enabled: Option<fn(&Ctx) -> bool>,
    /// The window's content, built again on every render.
    pub content: fn(&Ctx) -> El,
}

/// The accent a window set chooses: its seed in the light and the dark
/// appearance, as red, green and blue.
pub type Accent = fn(&Ctx) -> Option<((u8, u8, u8), (u8, u8, u8))>;

/// Run the application: start the engine, open the initial windows and
/// keep running until the last window closes.
pub fn run(def: AppDef, start: StartEngine) {
    // The definition lives as long as the process.
    let def: &'static AppDef = Box::leak(Box::new(def));
    host::run(def, start);
}
