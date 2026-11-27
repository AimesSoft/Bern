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

mod bar;
mod behaviors;
mod canvas;
mod chrome;
mod content;
mod controls;
mod ctx;
mod extra;
mod files;
mod filters;
mod form;
mod gestures;
mod host;
mod icons;
mod layout;
mod login_item;
mod look;
mod markdown;
mod marquee;
mod media;
mod media_executor;
mod menus;
mod motion;
mod native;
mod navigation;
mod overlay;
mod pictures;
mod pointer;
mod query;
mod scene;
mod scroll;
mod search;
mod session;
mod skeleton;
mod spin;
mod status;
mod surface;
mod svg;
mod table;
pub use bar::{BarEntry, BarMenu, bar_item, bar_menu, bar_submenu};
pub use login_item::login_model;
pub use marquee::{MarqueeProps, marquee};
pub use spin::Spin;
pub use status::{StatusItemDef, StatusMenu};
pub use svg::{SvgProps, svg};
pub use table::{TableColumnDef, TableProps, TableRowData, table};
pub mod text;
mod theme;
mod trace;
mod units;
pub mod value;
mod webpage;
mod windows;
pub use gestures::{SwipeProps, swipe};
pub use webpage::{WebPageProps, web_page};

pub use behaviors::{
    ColorPickerProps, ShortcutInputProps, ShortcutProps, SliderProps, color_picker,
    focus_on_appear, focus_region, pinch, screen_brightness, shortcut, shortcut_input, slider,
    timer, toast, watch,
};
pub use bern_ffi;
pub use bern_ffi::protocol;
pub use canvas::{
    Band, Fade, Op, PatternOp, SeriesOp, ShapeKind, ShapeOp, canvas_element as canvas,
};
pub use content::{
    Emphasis, Fit, TextProps, TextStyle, image, image_url, label, progress, progress_ring, text,
};
pub use controls::{
    ButtonProps, FilePrompt, IconButtonProps, InputKind, InputProps, PickerStyle, Prominence, Role,
    SwitchProps, SwitchStyle, button, button_with, drop_area, icon_button, language_picker, picker,
    switch, text_input, text_input_controlled,
};
pub use ctx::{
    Act, Ctx, Field, Model, act, session_pop, session_push_str, session_set, session_toggle,
};
pub use extra::{
    AlertActionSpec, AvatarProps, BannerProps, BottomSheetProps, DatePickerProps, NumberInputProps,
    ReorderableProps, SplitPaneProps, TimePickerProps, TooltipProps, TreeViewProps, alert, avatar,
    badge, banner, bottom_sheet, date_picker, disclosure, empty_state, number_input, refreshable,
    reorderable, split_pane, time_picker, tooltip, tree_view,
};
pub use filters::{blur_scope, glow};
pub use form::{FormProps, SectionProps, form, section};
pub use gpui;
pub use host::{Done, WindowOp, dispatch, dispatch_with, window_command};
pub use layout::{
    AccordionProps, Align, Flex, GridProps, Grow, Layers, Len, Pad, PreviewProps, Size, Style,
    accordion, adaptive, column, cover, divider, flow, grid, idle_area, inspector, lazy_grid,
    lazy_row, list, native_host, preview_list, reveal, row, scroll, spacer, stack, tab_selection,
    tabs, window_buttons,
};
pub use look::{ControlSize, ControlStyle, Look, Shape};
pub use media::{MediaOp, VideoProps, media, video};
pub use menus::{MenuEntry, MenuProps, context_menu, control_group, menu, menu_item};
pub use motion::{
    Curve, Edge, Motion, Pose, Transition, TransitionKind, eased, hero_key, leave, leaving, left,
    left_row, moving, reflow_origin, reflowed, rows, staggered,
};
pub use navigation::{
    detail, nav_link, navigation, navigation_reset, sidebar, sidebar_item, sidebar_selection,
    sidebar_title, split_view,
};
pub use overlay::{ModalProps, PopoverProps, SplitProps, modal, popover_button, split_button};
pub use pointer::{PointerEvents, pointer_events};
pub use query::{OwnedQuery, QueryProps, own_query, query};
pub use scene::{Anchor as RailAnchor, Place, Placement, positioned, rail, scale_box};
pub use scroll::{ScrollProps, Snap, scroll_items, scroll_key, scroll_view};
pub use search::{SearchItemProps, SearchProps, search, search_item, toolbar};
pub use session::{NativeControlActions, register_native};
pub use skeleton::{SkeletonProps, SkeletonShape, skeleton, skeleton_host, skeleton_host_one};

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
    /// The runtime palette JSON chosen at run time (`WindowSet(palette:)`).
    pub palette: Option<fn(&Ctx) -> String>,
    /// The appearance chosen at run time.
    pub appearance: Option<fn(&Ctx) -> Appearance>,
    /// The picture of `[app.icon]` among the resources, when it can be
    /// shown.
    pub app_icon: Option<&'static str>,
    /// The icon the window set chooses at run time (`WindowSet(icon:)`),
    /// as a picture among the resources.
    pub window_icon: Option<fn(&Ctx) -> Option<&'static str>>,
    /// The application's menus in the menu bar (`MenuBar`), built with the
    /// primary window's context.
    pub menu_bar: Option<fn(&Ctx) -> Vec<BarMenu>>,
    /// The application's system tray / menu bar status item (`StatusItem`).
    pub status_item: Option<StatusItemDef>,
    /// The icons the application shows, as SVG documents by name.
    pub icons: &'static [(&'static str, &'static str)],
    /// Text styles with weights (and desktop sizes) of their own, from
    /// `text { … }` in `style!`.
    pub text_weights: &'static [(&'static str, u16, Option<f32>)],
    /// Declared file kinds and their accepted extensions for `ui.set_file`
    /// and open panels.
    pub file_kinds: &'static [(&'static str, &'static [&'static str])],
    /// File kinds the engine itself handles via `[[app.opens]]`.
    pub opens: &'static str,
}

/// Callback invoked when the user chooses Save in a window close confirmation.
pub type SaveHandler = std::rc::Rc<dyn Fn(&Ctx, Done)>;

/// Outcome of a window's declared close check (`Window(confirm_close:, close_message:)`).
pub struct CloseCheck {
    pub needs_confirmation: bool,
    pub message: String,
    pub save: Option<SaveHandler>,
}

/// A window kind of the `WindowSet`.
pub struct WindowDef {
    pub id: &'static str,
    /// Text key of the title.
    pub title: &'static str,
    /// Opened when the application starts.
    pub open: bool,
    /// Application settings window (`role: settings`).
    pub settings: bool,
    /// Compact floating utility window (`role: utility`).
    pub utility: bool,
    /// Modal dialog window (`role: dialog`).
    pub dialog: bool,
    /// Name of a `bern::overlay` this utility window anchors beside.
    pub overlay: Option<&'static str>,
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
    /// Declared close check, run when closing is requested.
    pub close: Option<fn(&Ctx) -> CloseCheck>,
    /// Engine state fields read by this window and its components.
    pub engine_fields: &'static [usize],
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
