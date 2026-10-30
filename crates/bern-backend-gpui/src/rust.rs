//! Rust source of a GPUI application's interface: one function per
//! component and per window that builds its elements from `bern-gpui`, run
//! again on every render, and the [`AppDef`](bern_gpui::AppDef) the
//! runtime starts from.
//!
//! Control flow is Rust's own: `if`, `if let`, `match` and the rows of a
//! `ForEach` push the elements they show onto their container's list.
//! The generator realizes only what this target lists; everything else is
//! reported where it is declared, so a build never shows a placeholder in
//! its place.

use bern_ir::style::{ColorRef, ControlSize, StyleEffect, StyleState};
use bern_ir::{
    Action, Application, BinaryOp, Binding, ComponentArg, ComponentDef, Element, Expr, ExprKind,
    Node, ParamKind, Presentation, Prop, SessionOp, SourceLocation, Text, WindowChrome, WindowDef,
    WindowInitial, WindowRole,
};
use bern_protocol::{Type, Value};
use bern_target_api::Diagnostic;
use bern_target_api::glue::value_expr;
use std::cell::RefCell;
use std::collections::BTreeSet;

/// Properties every element may carry: the entry of a feature contract,
/// the light or dark appearance of its content, and the motion
/// properties, which this target shows without motion (the shared check
/// warns of each).
const COMMON: &[&str] = &[
    "feature",
    "appearance",
    "appear",
    "appear_delay",
    "disappear",
    "reflow",
    "hero",
    "hero_key",
    "animate",
    "scroll_scale",
    "scroll_opacity",
    "scroll_origin_x",
    "scroll_origin_y",
    "nudge",
    "nudge_key",
    "nudge_scale",
    "reduce_motion",
    "spin",
    "spin_period",
    "stagger",
    "idle_area",
    // Where a child stands on a `Rail`.
    "at",
    "from",
    "to",
    "reveal",
];

/// The components this target builds, each with the properties it shows;
/// `id` is a control's identity.
pub const COMPONENTS: &[(&str, &[&str])] = &[
    (
        "Column",
        &[
            "gap",
            "padding",
            "padding_x",
            "padding_y",
            "width",
            "height",
            "align",
            "variant",
        ],
    ),
    (
        "Row",
        &[
            "gap",
            "padding",
            "padding_x",
            "padding_y",
            "width",
            "height",
            "align",
            "variant",
        ],
    ),
    ("Flow", &["gap"]),
    (
        "Grid",
        &[
            "min_width",
            "gap",
            "row_gap",
            "padding",
            "padding_x",
            "padding_y",
            "cell_ratio",
            "cell_extra",
            "masonry",
        ],
    ),
    (
        "Stack",
        &[
            "variant",
            "enabled",
            "align",
            "width",
            "height",
            "under_bars",
        ],
    ),
    (
        "ScrollView",
        &[
            "axis",
            "indicators",
            "snap",
            "position",
            "arrival",
            "open_at",
            "open_center",
        ],
    ),
    ("Spacer", &[]),
    ("Divider", &[]),
    (
        "Text",
        &[
            "content",
            "style",
            "emphasis",
            "lines",
            "font_size",
            "color",
            "line_height",
            "markdown",
            "mark",
        ],
    ),
    ("Label", &["title", "icon", "style", "font_size", "display"]),
    (
        "Image",
        &[
            "source",
            "dark_source",
            "label",
            "width",
            "height",
            "corner",
            "fit",
            "rendering",
            "aspect",
        ],
    ),
    ("Progress", &["value", "total", "label"]),
    (
        "Button",
        &[
            "id",
            "label",
            "icon",
            "role",
            "prominence",
            "enabled",
            "width",
            "pick",
            "files",
            "shortcut",
            "shape",
            "height",
            "variant",
            "clears",
            "default_focus",
        ],
    ),
    (
        "IconButton",
        &[
            "id",
            "icon",
            "label",
            "enabled",
            "pick",
            "files",
            "prominence",
            "height",
            "font_size",
            "variant",
            "shortcut",
            "default_focus",
        ],
    ),
    (
        "Switch",
        &[
            "id",
            "label",
            "value",
            "checked",
            "style",
            "label_hidden",
            "shape",
            "height",
            "icon",
        ],
    ),
    (
        "Picker",
        &[
            "id",
            "label",
            "value",
            "selected",
            "label_hidden",
            "style",
            "shape",
            "height",
        ],
    ),
    ("LanguagePicker", &["id", "label"]),
    (
        "TextInput",
        &[
            "id",
            "label",
            "label_hidden",
            "value",
            "prompt",
            "kind",
            "submit",
            "clear_key",
        ],
    ),
    (
        "PopoverButton",
        &[
            "id",
            "label",
            "icon",
            "presented",
            "open",
            "panel_appearance",
        ],
    ),
    ("Modal", &["width", "height", "cover"]),
    (
        "Form",
        &[
            "gap",
            "title",
            "variant",
            "row_height",
            "padding_x",
            "padding_y",
        ],
    ),
    (
        "Section",
        &["title", "footer", "style", "padding_x", "padding_y"],
    ),
    ("Tabs", &["id", "selection"]),
    ("Tab", &["value", "label", "icon", "display"]),
    ("TabAccessory", &["shown"]),
    ("Drop", &["id", "label", "value", "files", "over"]),
    ("Adaptive", &["id"]),
    ("Layout", &["size"]),
    ("ForEach", &["items", "key"]),
    (
        "Query",
        &["id", "query", "loading", "empty", "failed", "icon"],
    ),
    ("Toolbar", &[]),
    (
        "Video",
        &[
            "id", "source", "player", "label", "controls", "fit", "sizing", "corner",
        ],
    ),
    ("FocusRegion", &[]),
    ("Pinch", &["id", "label", "value", "min", "max"]),
    (
        "SplitButton",
        &[
            "id",
            "label",
            "value",
            "checked",
            "icon",
            "shape",
            "height",
            "more_label",
            "shortcut",
        ],
    ),
    (
        "PreviewList",
        &["items", "key", "item_height", "gap", "more_height"],
    ),
    (
        "Accordion",
        &[
            "count",
            "selected",
            "collapsed_width",
            "height",
            "gap",
            "corner",
        ],
    ),
    (
        "Search",
        &[
            "id",
            "value",
            "prompt",
            "shortcut",
            "style",
            "panel_prompt",
            "width",
            "height",
            "font_size",
            "suggest",
        ],
    ),
    (
        "SearchItem",
        &[
            "id",
            "label",
            "icon",
            "detail",
            "hint",
            "navigation",
            "pick",
            "files",
        ],
    ),
    ("Slot", &["id"]),
    ("Canvas", &["width", "height", "label"]),
    (
        "Shape",
        &[
            "kind",
            "x",
            "y",
            "width",
            "height",
            "corner",
            "fill",
            "fill_from",
            "fade",
            "stroke",
            "stroke_width",
            "dash",
            "dash_gap",
            "opacity",
        ],
    ),
    (
        "Series",
        &[
            "values",
            "kind",
            "max",
            "fill",
            "stroke",
            "stroke_width",
            "opacity",
        ],
    ),
    ("Pattern", &["name", "opacity"]),
    (
        "Menu",
        &[
            "id",
            "label",
            "hide_disabled",
            "icon",
            "style",
            "shape",
            "height",
        ],
    ),
    (
        "MenuItem",
        &[
            "id", "label", "icon", "role", "enabled", "shortcut", "pick", "files",
        ],
    ),
    ("PickerOption", &["value", "label"]),
    ("ControlGroup", &["label", "height"]),
    (
        "Slider",
        &[
            "id",
            "label",
            "label_hidden",
            "value",
            "current",
            "continuous",
            "min",
            "max",
            "step",
            "min_icon",
            "max_icon",
            "tint",
        ],
    ),
    ("Shortcut", &["key", "enabled"]),
    (
        "ShortcutInput",
        &[
            "id",
            "label",
            "label_hidden",
            "current",
            "prompt",
            "empty",
            "enabled",
        ],
    ),
    ("Toast", &["content", "icon", "key", "duration"]),
    ("Timer", &["interval", "enabled"]),
    ("Watch", &["value"]),
    ("ScaleBox", &["width", "height"]),
    ("Positioned", &["x", "y", "width", "height", "bleed"]),
    ("Rail", &["axis", "anchor"]),
    ("Navigation", &["path", "id"]),
    ("NavLink", &["id", "route"]),
    ("Destination", &[]),
    ("SplitView", &[]),
    ("Sidebar", &["selection", "width", "id"]),
    (
        "SidebarItem",
        &["value", "label", "icon", "badge", "variant"],
    ),
    ("SidebarHeader", &["role"]),
    ("SidebarFooter", &[]),
    ("Detail", &["titled"]),
];

/// The names [`COMPONENTS`] lists, with the window catalog's.
pub const COMPONENT_NAMES: &[&str] = &[
    "WindowSet",
    "Window",
    "Column",
    "Row",
    "Flow",
    "Grid",
    "Stack",
    "ScrollView",
    "Spacer",
    "Divider",
    "Text",
    "Label",
    "Image",
    "Progress",
    "Button",
    "IconButton",
    "Switch",
    "Picker",
    "LanguagePicker",
    "TextInput",
    "PopoverButton",
    "Modal",
    "Form",
    "Section",
    "Tabs",
    "Tab",
    "TabAccessory",
    "Drop",
    "Adaptive",
    "Layout",
    "ForEach",
    "Query",
    "Toolbar",
    "Video",
    "Search",
    "SearchItem",
    "FocusRegion",
    "Pinch",
    "SplitButton",
    "PreviewList",
    "Accordion",
    "Slot",
    "Canvas",
    "Shape",
    "Series",
    "Pattern",
    "Menu",
    "MenuItem",
    "PickerOption",
    "ControlGroup",
    "Slider",
    "Shortcut",
    "ShortcutInput",
    "Toast",
    "Timer",
    "Watch",
    "ScaleBox",
    "Positioned",
    "Rail",
    "Navigation",
    "NavLink",
    "Destination",
    "SplitView",
    "Sidebar",
    "SidebarItem",
    "SidebarHeader",
    "SidebarFooter",
    "Detail",
];

/// Icons the runtime draws itself, whatever the application declares.
const RUNTIME_ICONS: &[&str] = &[
    "checkmark",
    "chevron.down",
    "chevron.left",
    "chevron.right",
    "exclamationmark.triangle",
    "magnifyingglass",
    "tray",
    "xmark",
    "xmark.circle.fill",
];

fn string(text: &str) -> String {
    format!("{text:?}")
}

fn float(value: f64) -> String {
    format!("{:?}", value as f32)
}

/// The direction a container lays its children out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Column,
    Row,
}

/// Whether a node takes the space it is given, across and down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Greed {
    width: bool,
    height: bool,
}

impl Greed {
    fn or(self, other: Greed) -> Greed {
        Greed {
            width: self.width || other.width,
            height: self.height || other.height,
        }
    }

    /// How an element taking this space grows in a container of `axis`.
    fn grow(self, axis: Axis) -> String {
        let (flex, width, height) = match axis {
            Axis::Column => (self.height, self.width, false),
            Axis::Row => (self.width, false, self.height),
        };
        format!("b::Grow {{ flex: {flex}, width: {width}, height: {height} }}")
    }
}

pub struct Generator<'a> {
    app: &'a Application,
    /// The platform whose `style!` rules apply.
    platform: &'a str,
    diagnostics: RefCell<Vec<Diagnostic>>,
    icons: RefCell<BTreeSet<String>>,
}

impl<'a> Generator<'a> {
    pub fn new(app: &'a Application, platform: &'a str) -> Self {
        Self {
            app,
            platform,
            diagnostics: RefCell::new(Vec::new()),
            icons: RefCell::new(RUNTIME_ICONS.iter().map(|icon| icon.to_string()).collect()),
        }
    }

    fn unsupported(&self, what: String, at: Option<&SourceLocation>) {
        let diagnostic = Diagnostic::error(format!("the GPUI target does not show {what} yet"));
        self.diagnostics.borrow_mut().push(match at {
            Some(source) => diagnostic.at(source.clone()),
            None => diagnostic,
        });
    }

    /// What the generated source would leave out.
    pub fn check(&self) -> Vec<Diagnostic> {
        self.generate(0, "descriptor");
        self.diagnostics.take()
    }

    pub fn generate(&self, fingerprint: u64, descriptor: &str) -> String {
        let app = self.app;
        let mut out = String::new();
        let mut line = |text: &str| {
            out.push_str(text);
            out.push('\n');
        };
        line("// Generated by Bern from the Rust declarations. Do not edit.");
        line(
            "#![allow(unused_imports, unused_variables, unused_mut, unused_parens, unused_braces, dead_code, clippy::all)]",
        );
        line("");
        line("use bern_gpui as b;");
        line("use bern_gpui::protocol::Value as V;");
        line("use bern_gpui::value as rt;");
        line("");
        line("// Components.");
        for component in &app.components {
            line(&self.component(component));
        }
        line("// Windows.");
        for window in &app.windows.windows {
            line(&self.window_content(window));
        }
        line(&self.styles());
        line(&self.window_theme());
        line("/// Strings of every supported language.");
        line("static STRINGS: b::text::StringTable = &[");
        for (language, table) in bern_target_api::texts::tables(app) {
            line(&format!("    ({}, &[", string(&language)));
            for (key, text) in table {
                line(&format!("        ({}, {}),", string(&key), string(&text)));
            }
            line("    ]),");
        }
        line("];");
        line("");
        line(&self.icon_table());
        line(&self.app_def(fingerprint, descriptor));
        self.window_catalog();
        out
    }

    // MARK: - Windows and the application

    fn component(&self, component: &ComponentDef) -> String {
        // Controls of the body are named after the component; the element
        // the body is keeps the space its caller gives it.
        format!(
            "pub fn c_{}(c: &b::Ctx) -> b::El {{\n    {}\n}}\n",
            component.name,
            self.root(&component.body)
        )
    }

    fn window_content(&self, window: &WindowDef) -> String {
        let mut out = format!(
            "fn window_{}(c: &b::Ctx) -> b::El {{\n    {}\n}}\n",
            window.id,
            self.root(&window.content)
        );
        for (name, expr) in [
            ("immersive", &window.immersive),
            ("idle_enabled", &window.idle_enabled),
        ] {
            if let Some(expr) = expr {
                out.push_str(&format!(
                    "fn window_{}_{name}(c: &b::Ctx) -> bool {{\n    {}\n}}\n",
                    window.id,
                    self.condition(expr)
                ));
            }
        }
        out
    }

    /// The element a component's or window's body is, built in the
    /// caller's context; control flow at the top becomes a column.
    fn root(&self, node: &Node) -> String {
        match node {
            Node::Element(element) if element.kind != "ForEach" => self
                .element(element, "c")
                .unwrap_or_else(|| "b::column(c, b::Flex::default(), Vec::new())".into()),
            _ => format!(
                "b::column(c, b::Flex::default(), {})",
                self.children(std::slice::from_ref(node), Axis::Column)
            ),
        }
    }

    /// Window properties beyond the size, the title and the chrome are not
    /// shown yet.
    fn window_catalog(&self) {
        let catalog = &self.app.windows;
        if catalog.palette.is_some() {
            self.unsupported("`WindowSet(palette:)`".into(), None);
        }
        for window in &catalog.windows {
            let mut unsupported = Vec::new();
            if window.presentation != Presentation::Auto {
                unsupported.push("presentation");
            }
            if window.role != WindowRole::Standard {
                unsupported.push("role");
            }
            if window.close.is_some() {
                unsupported.push("close");
            }
            // A desktop window has no orientation to turn (`landscape`).
            for property in unsupported {
                self.unsupported(format!("`Window(id: {}, {property}:)`", window.id), None);
            }
        }
    }

    /// The accent and appearance the application chooses at run time.
    fn window_theme(&self) -> String {
        let catalog = &self.app.windows;
        let mut out = String::new();
        if let Some(expr) = &catalog.accent {
            let arms: Vec<String> = self
                .enum_variants(&expr.ty)
                .into_iter()
                .enumerate()
                .filter_map(|(index, name)| {
                    let accent = self.app.style.accent(&name)?;
                    match &accent.color {
                        ColorRef::Fixed { light, dark } => Some(format!(
                            "Some({index}) => Some((({}, {}, {}), ({}, {}, {}))),",
                            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
                        )),
                        ColorRef::Role { role } => {
                            self.unsupported(format!("the accent `{name}` of role `{role}`"), None);
                            None
                        }
                    }
                })
                .collect();
            out.push_str(&format!(
                "fn window_accent(c: &b::Ctx) -> Option<((u8, u8, u8), (u8, u8, u8))> {{\n    match rt::tag(&{}) {{\n        {}\n        _ => None,\n    }}\n}}\n",
                self.expr(expr, None),
                arms.join("\n        ")
            ));
        }
        if let Some(expr) = &catalog.appearance {
            let arms: Vec<String> = self
                .enum_variants(&expr.ty)
                .into_iter()
                .enumerate()
                .map(|(index, name)| {
                    let appearance = match name.as_str() {
                        "Light" => "Light",
                        "Dark" => "Dark",
                        _ => "System",
                    };
                    format!("Some({index}) => b::Appearance::{appearance},")
                })
                .collect();
            out.push_str(&format!(
                "fn window_appearance(c: &b::Ctx) -> b::Appearance {{\n    match rt::tag(&{}) {{\n        {}\n        _ => b::Appearance::System,\n    }}\n}}\n",
                self.expr(expr, None),
                arms.join("\n        ")
            ));
        }
        // The icon the application shows, from the pictures it ships.
        if let Some(expr) = &catalog.icon {
            let pictures = crate::icon_pictures(self.app, self.platform);
            let picture = |name: &str| {
                pictures
                    .iter()
                    .find(|(shipped, _)| {
                        std::path::Path::new(shipped)
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            == Some(name)
                    })
                    .map(|(shipped, _)| format!("Some({})", string(shipped)))
                    .unwrap_or_else(|| "None".into())
            };
            let arms: Vec<String> = bern_target_api::icon::choices(self.app)
                .into_iter()
                .enumerate()
                .map(|(index, (_, alternate))| {
                    format!(
                        "Some({index}) => {},",
                        picture(alternate.as_deref().unwrap_or("app"))
                    )
                })
                .collect();
            out.push_str(&format!(
                "fn window_icon(c: &b::Ctx) -> Option<&'static str> {{\n    match rt::tag(&{}) {{\n        {}\n        _ => {},\n    }}\n}}\n",
                self.expr(expr, None),
                arms.join("\n        "),
                picture("app")
            ));
        }
        out
    }

    fn enum_variants(&self, ty: &Type) -> Vec<String> {
        match ty {
            Type::Enum { name } => self
                .app
                .types
                .enum_def(name)
                .map(|def| {
                    def.variants
                        .iter()
                        .map(|variant| variant.name.clone())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The icons the application shows, as SVG documents from the Web
    /// target's set.
    fn icon_table(&self) -> String {
        let mut entries = Vec::new();
        for (index, name) in self.icons.borrow().iter().enumerate() {
            match bern_backend_web::icons::svg(name) {
                Some(markup) => {
                    let markup = markup.replace(
                        bern_backend_web::icons::ID_PLACEHOLDER,
                        &format!("bern-icon-{index}"),
                    );
                    let document = format!(
                        r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                        markup.replace("currentColor", "black")
                    );
                    entries.push(format!("    ({}, {}),", string(name), string(&document)));
                }
                None => self.unsupported(format!("the icon `{name}`"), None),
            }
        }
        format!(
            "/// The icons the application shows.\nstatic ICONS: &[(&str, &str)] = &[\n{}\n];\n",
            entries.join("\n")
        )
    }

    fn app_def(&self, fingerprint: u64, descriptor: &str) -> String {
        let app = self.app;
        let languages: Vec<_> = app
            .languages
            .supported
            .iter()
            .map(|language| string(language))
            .collect();
        let size = |value: Option<f64>| match value {
            Some(value) => format!("Some({value:?})"),
            None => "None".into(),
        };
        let windows: Vec<String> = app
            .windows
            .windows
            .iter()
            .map(|window| {
                format!(
                    "b::WindowDef {{ id: {}, title: {}, open: {}, width: {}, height: {}, min_width: {}, min_height: {}, unified: {}, idle_after: {}, immersive: {}, idle_enabled: {}, content: window_{} }},",
                    string(&window.id),
                    string(&window.title.key),
                    window.initial == WindowInitial::Open,
                    size(window.width),
                    size(window.height),
                    size(window.min_width),
                    size(window.min_height),
                    window.chrome == WindowChrome::Unified,
                    size(window.idle_after),
                    match window.immersive {
                        Some(_) => format!("Some(window_{}_immersive)", window.id),
                        None => "None".into(),
                    },
                    match window.idle_enabled {
                        Some(_) => format!("Some(window_{}_idle_enabled)", window.id),
                        None => "None".into(),
                    },
                    window.id,
                )
            })
            .collect();
        let sessions: Vec<String> = app
            .sessions
            .iter()
            .map(|session| {
                let fields: Vec<_> = session
                    .fields
                    .iter()
                    .map(|field| {
                        format!(
                            "({}, {}, {})",
                            string(&field.name),
                            bern_target_api::glue::type_expr(&field.ty),
                            value_expr(&field.initial)
                        )
                    })
                    .collect();
                format!(
                    "b::SessionDef {{ name: {}, fields: vec![{}] }},",
                    string(&session.name),
                    fields.join(", ")
                )
            })
            .collect();
        let brand = match app.theme.brand_seed {
            Some(seed) => format!("Some(({}, {}, {}))", seed.red, seed.green, seed.blue),
            None => "None".into(),
        };
        let layouts = app.layouts;
        format!(
            "/// The application, as the runtime sees it.
pub fn app() -> b::AppDef {{
    b::AppDef {{
        id: {id},
        bundle_id: {bundle_id},
        name: \"app.name\",
        version: {version},
        fingerprint: 0x{fingerprint:016x},
        languages: &[{languages}],
        default_language: {default_language},
        strings: STRINGS,
        descriptor: ({descriptor}).clone(),
        types: {types},
        sessions: vec![
            {sessions}
        ],
        windows: vec![
            {windows}
        ],
        primary: {primary},
        compact_below: {compact},
        medium_below: {medium},
        expanded_below: {expanded},
        brand_seed: {brand},
        accent: {accent},
        appearance: {appearance},
        app_icon: {app_icon},
        window_icon: {window_icon},
        icons: ICONS,
        text_weights: &[{text_weights}],
    }}
}}
",
            id = string(&app.app.id),
            bundle_id = string(&app.app.bundle_id),
            version = string(&app.app.version),
            languages = languages.join(", "),
            default_language = string(&app.languages.default),
            types = bern_target_api::glue::types_expr(&app.types),
            sessions = sessions.join("\n            "),
            windows = windows.join("\n            "),
            primary = string(&app.windows.primary),
            compact = size(layouts.map(|layouts| layouts.compact_below)),
            medium = size(layouts.and_then(|layouts| layouts.medium_below)),
            expanded = size(layouts.and_then(|layouts| layouts.expanded_below)),
            accent = if app.windows.accent.is_some() {
                "Some(window_accent)"
            } else {
                "None"
            },
            text_weights = app
                .style
                .text_weights
                .iter()
                .map(|weight| format!(
                    "({}, {}, {})",
                    string(&weight.style),
                    match weight.weight.as_str() {
                        "ultralight" => 100,
                        "thin" => 200,
                        "light" => 300,
                        "regular" => 400,
                        "medium" => 500,
                        "semibold" => 600,
                        "bold" => 700,
                        "heavy" => 800,
                        "black" => 900,
                        _ => 400,
                    },
                    match weight.size {
                        Some(size) => format!("Some({})", float(size)),
                        None => "None".into(),
                    }
                ))
                .collect::<Vec<_>>()
                .join(", "),
            appearance = if app.windows.appearance.is_some() {
                "Some(window_appearance)"
            } else {
                "None"
            },
            app_icon = crate::icon_pictures(app, self.platform)
                .into_iter()
                .find(|(shipped, _)| shipped.starts_with("icons/app."))
                .map(|(shipped, _)| format!("Some({})", string(&shipped)))
                .unwrap_or_else(|| "None".into()),
            window_icon = if app.windows.icon.is_some() {
                "Some(window_icon)"
            } else {
                "None"
            },
        )
    }

    // MARK: - Styles

    /// The `style!` rules the declarations use: a static for each surface
    /// (`Stack`, `Form`, `Sidebar`, `Detail`, a `PopoverButton`'s panel) and
    /// each control (`Button`, `IconButton`, `SidebarItem`) kind and variant.
    fn styles(&self) -> String {
        let mut out = String::new();
        for (index, (kind, variant)) in self.styled("surface").iter().enumerate() {
            let Some(resolved) =
                bern_target_api::style::resolve(self.app, self.platform, kind, variant.as_deref())
            else {
                continue;
            };
            let mut fields = Vec::new();
            for effect in &resolved.base {
                match effect {
                    StyleEffect::Background { color } => {
                        fields.push(format!("background: Some({})", color_expr(color)))
                    }
                    StyleEffect::Foreground { color } => {
                        fields.push(format!("foreground: Some({})", color_expr(color)))
                    }
                    StyleEffect::Corner { radius } => {
                        fields.push(format!("corner: Some({})", float(*radius)))
                    }
                    StyleEffect::Shadow { radius, y, opacity } => fields.push(format!(
                        "shadow: Some(({}, {}, {}))",
                        float(*radius),
                        float(*y),
                        float(*opacity)
                    )),
                    // The shared check reports what the target cannot show.
                    _ => {}
                }
            }
            fields.push("..b::Style::NONE".into());
            out.push_str(&format!(
                "static STYLE_{index}: b::Style = b::Style {{ {} }};\n",
                fields.join(", ")
            ));
        }
        for (index, (kind, variant)) in self.styled("control").iter().enumerate() {
            let Some(resolved) =
                bern_target_api::style::resolve(self.app, self.platform, kind, variant.as_deref())
            else {
                continue;
            };
            let mut fields = vec![format!("base: {}", look(&resolved.base))];
            for effect in &resolved.base {
                match effect {
                    StyleEffect::Corner { radius } => {
                        fields.push(format!("corner: Some({})", float(*radius)))
                    }
                    StyleEffect::Padding { length } => {
                        fields.push(format!("padding: Some({})", float(*length)))
                    }
                    StyleEffect::Size { size } => fields.push(format!(
                        "size: Some(b::ControlSize::{})",
                        match size {
                            ControlSize::Small => "Small",
                            ControlSize::Regular => "Regular",
                            ControlSize::Large => "Large",
                            ControlSize::ExtraLarge => "ExtraLarge",
                        }
                    )),
                    _ => {}
                }
            }
            for (state, effects) in &resolved.states {
                let name = match state {
                    StyleState::Hover => "hover",
                    StyleState::Pressed => "pressed",
                    StyleState::Disabled => "disabled",
                    StyleState::Selected => "selected",
                    StyleState::Focus => continue,
                };
                fields.push(format!("{name}: Some({})", look(effects)));
            }
            fields.push("..b::ControlStyle::NONE".into());
            out.push_str(&format!(
                "static CONTROL_STYLE_{index}: b::ControlStyle = b::ControlStyle {{ {} }};\n",
                fields.join(", ")
            ));
        }
        out
    }

    /// The kinds and variants of styled elements in the declarations, in a
    /// stable order: `surface` ones or `control` ones.
    fn styled(&self, family: &str) -> Vec<(String, Option<String>)> {
        let kinds: &[&str] = if family == "surface" {
            &["Stack", "Form", "Sidebar", "Detail", "PopoverButton"]
        } else {
            &["Button", "IconButton", "SidebarItem"]
        };
        let mut used = BTreeSet::new();
        self.app.walk(&mut |node, _| {
            if let Node::Element(element) = node
                && kinds.contains(&element.kind.as_str())
            {
                used.insert((
                    element.kind.clone(),
                    element.keyword("variant").map(str::to_string),
                ));
            }
        });
        used.into_iter().collect()
    }

    fn style_index(&self, family: &str, element: &Element) -> Option<usize> {
        let wanted = (
            element.kind.clone(),
            element.keyword("variant").map(str::to_string),
        );
        bern_target_api::style::resolve(
            self.app,
            self.platform,
            &element.kind,
            element.keyword("variant"),
        )?;
        self.styled(family).iter().position(|used| *used == wanted)
    }

    fn surface_style(&self, _kind: &str, element: &Element) -> String {
        match self.style_index("surface", element) {
            Some(index) => format!("Some(&STYLE_{index})"),
            None => "None".into(),
        }
    }

    fn stack_style(&self, element: &Element) -> String {
        self.surface_style("Stack", element)
    }

    fn control_style(&self, element: &Element) -> String {
        match self.style_index("control", element) {
            Some(index) => format!("Some(&CONTROL_STYLE_{index})"),
            None => "None".into(),
        }
    }

    /// A button's key combination: written, or following state.
    fn shortcut(&self, element: &Element) -> String {
        match element.prop("shortcut") {
            Some(Prop::Keyword { value }) => format!("Some({}.to_string())", string(value)),
            Some(Prop::Expr { expr }) => format!(
                "Some(rt::plain(&{}))",
                self.expr(expr, Some(&element.source))
            ),
            _ => "None".into(),
        }
    }

    // MARK: - Space

    /// Whether a node takes the space its container gives.
    fn greed(&self, node: &Node, axis: Axis) -> Greed {
        match node {
            Node::Element(element) => self.element_greed(element, axis),
            Node::Component(reference) => self
                .app
                .component(&reference.component)
                .map(|component| self.greed(&component.body, axis))
                .unwrap_or_default(),
            Node::If(node) => node
                .branches
                .iter()
                .flat_map(|(_, body)| body.iter())
                .chain(node.otherwise.iter())
                .fold(Greed::default(), |greed, node| {
                    greed.or(self.greed(node, axis))
                }),
            Node::IfLet(node) => node
                .then
                .iter()
                .chain(node.otherwise.iter())
                .fold(Greed::default(), |greed, node| {
                    greed.or(self.greed(node, axis))
                }),
            Node::Match(node) => node
                .arms
                .iter()
                .flat_map(|arm| arm.body.iter())
                .fold(Greed::default(), |greed, node| {
                    greed.or(self.greed(node, axis))
                }),
        }
    }

    fn children_greed(&self, nodes: &[Node], axis: Axis) -> Greed {
        nodes.iter().fold(Greed::default(), |greed, node| {
            let spacer = matches!(node, Node::Element(element) if element.kind == "Spacer");
            let own = if spacer {
                Greed {
                    width: axis == Axis::Row,
                    height: axis == Axis::Column,
                }
            } else {
                self.greed(node, axis)
            };
            greed.or(own)
        })
    }

    fn element_greed(&self, element: &Element, parent: Axis) -> Greed {
        let fixed = |name: &str| element.prop(name).is_some();
        let greed = match element.kind.as_str() {
            // A shelf scrolling sideways is as tall as what it shows.
            "ScrollView" if element.keyword("axis") == Some("horizontal") => Greed {
                width: true,
                height: false,
            },
            "ScrollView" | "Form" | "Tabs" | "ScaleBox" | "SplitView" | "Navigation" => Greed {
                width: true,
                height: true,
            },
            "Divider" => Greed {
                width: parent == Axis::Column,
                height: parent == Axis::Row,
            },
            "TextInput" | "Progress" | "Section" | "NavLink" | "SearchItem" => Greed {
                width: true,
                height: false,
            },
            "Picker" => Greed {
                width: element.keyword("style") == Some("segmented")
                    || element.prop("label_hidden").is_none(),
                height: false,
            },
            "Switch" => Greed {
                width: element.keyword("style") != Some("checkbox"),
                height: false,
            },
            "Button" => Greed {
                width: element.keyword("width") == Some("fill"),
                height: false,
            },
            "Video" => Greed {
                width: true,
                height: element.keyword("sizing") == Some("fill"),
            },
            "Toolbar" | "Search" => Greed {
                width: true,
                height: false,
            },
            "Accordion" => Greed {
                width: true,
                height: element.prop("height").is_none(),
            },
            "PreviewList" => Greed {
                width: false,
                height: true,
            },
            "Column" | "Drop" | "Adaptive" | "Layout" | "ForEach" | "Query" | "FocusRegion"
            | "Pinch" => self.children_greed(&element.children, Axis::Column),
            "Row" | "Flow" => self.children_greed(&element.children, Axis::Row),
            "Rail" => Greed {
                width: element.keyword("axis") != Some("vertical"),
                height: element.keyword("axis") == Some("vertical"),
            },
            // As large as its largest layer: it fills what any layer fills.
            "Stack" => element
                .children
                .iter()
                .fold(Greed::default(), |greed, layer| {
                    greed.or(self.greed(layer, Axis::Column))
                }),
            _ => Greed::default(),
        };
        Greed {
            width: greed.width && !fixed("width"),
            height: greed.height && !fixed("height"),
        }
    }

    // MARK: - Nodes

    /// The elements `nodes` show, as an expression for a `Vec<b::El>` in a
    /// container of `axis`.
    fn children(&self, nodes: &[Node], axis: Axis) -> String {
        let mut body = String::new();
        for node in nodes {
            self.push(&mut body, node, axis);
        }
        format!("{{ let mut v: Vec<b::El> = Vec::new(); {body} v }}")
    }

    /// Statements that push the elements of `node` onto `v`.
    fn push(&self, out: &mut String, node: &Node, axis: Axis) {
        match node {
            Node::Element(element) if element.kind == "ForEach" => {
                self.for_each(out, element, axis)
            }
            Node::Element(element) if element.kind == "Slot" => {
                // Modules contribute to slots; an application without them
                // shows nothing there.
                if !element.children.is_empty() {
                    self.unsupported("contributions to a `Slot`".into(), Some(&element.source));
                }
            }
            Node::Element(element) => {
                let ctx = format!("&c.grown({})", self.element_greed(element, axis).grow(axis));
                if let Some(code) = self.element(element, &ctx) {
                    out.push_str(&format!("v.push({code}); "));
                }
            }
            Node::Component(reference) => {
                let Some(component) = self.app.component(&reference.component) else {
                    return;
                };
                let mut params = Vec::new();
                for param in &component.params {
                    if !matches!(param.kind, ParamKind::Value { .. }) {
                        continue;
                    }
                    let value = reference.args.iter().find(|(name, _)| *name == param.name);
                    if let Some((_, ComponentArg::Value { expr })) = value {
                        params.push(format!(
                            "({}, {})",
                            string(&param.name),
                            self.expr(expr, Some(&reference.source))
                        ));
                    }
                }
                out.push_str(&format!(
                    "v.push(c_{}(&c.child({}, vec![{}]).grown({}))); ",
                    component.name,
                    string(&component.name),
                    params.join(", "),
                    self.greed(node, axis).grow(axis)
                ));
            }
            Node::If(node) => {
                for (index, (condition, body)) in node.branches.iter().enumerate() {
                    if index > 0 {
                        out.push_str("else ");
                    }
                    out.push_str(&format!("if {} {{ ", self.condition(condition)));
                    for node in body {
                        self.push(out, node, axis);
                    }
                    out.push_str("} ");
                }
                if !node.otherwise.is_empty() {
                    out.push_str("else { ");
                    for node in &node.otherwise {
                        self.push(out, node, axis);
                    }
                    out.push_str("} ");
                }
            }
            Node::IfLet(node) => {
                out.push_str(&format!(
                    "if let Some(x) = rt::some({}) {{ let c = &c.with_local({}, x); ",
                    self.expr(&node.value, None),
                    string(&node.var)
                ));
                for node in &node.then {
                    self.push(out, node, axis);
                }
                out.push_str("} ");
                if !node.otherwise.is_empty() {
                    out.push_str("else { ");
                    for node in &node.otherwise {
                        self.push(out, node, axis);
                    }
                    out.push_str("} ");
                }
            }
            Node::Match(node) => {
                out.push_str(&format!(
                    "{{ let m = {}; match rt::tag(&m) {{ ",
                    self.expr(&node.value, None)
                ));
                let mut wildcard = false;
                for arm in &node.arms {
                    match arm.variant {
                        Some(variant) => out.push_str(&format!("Some({variant}) => {{ ")),
                        None => {
                            wildcard = true;
                            out.push_str("_ => { ");
                        }
                    }
                    for binding in &arm.bindings {
                        out.push_str(&format!(
                            "let c = &c.with_local({}, rt::payload(m.clone(), {})); ",
                            string(&binding.name),
                            binding.field
                        ));
                    }
                    for node in &arm.body {
                        self.push(out, node, axis);
                    }
                    out.push_str("} ");
                }
                if !wildcard {
                    out.push_str("_ => {} ");
                }
                out.push_str("} } ");
            }
        }
    }

    fn for_each(&self, out: &mut String, element: &Element, axis: Axis) {
        let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
            element.expr("items"),
            &element.template,
            element.prop("key"),
        ) else {
            self.unsupported("this `ForEach`".into(), Some(&element.source));
            return;
        };
        // Each row's elements; inside a scroll view that rests on items,
        // the runtime wraps them to note where they are.
        out.push_str(&format!(
            "for item in rt::as_list({}) {{ let key = rt::at(item.clone(), &[{field}]); let c = &c.with_local({}, item).with_item(&key); let row = {{ let mut v: Vec<b::El> = Vec::new(); ",
            self.expr(items, Some(&element.source)),
            string(&template.var)
        ));
        for node in &element.children {
            self.push(out, node, axis);
        }
        out.push_str("v }; b::scroll_items(c, key, row, &mut v); } ");
    }

    /// An element's expression, built in context `ctx`; the element's own
    /// context, which its children build in too, adds its local appearance
    /// and a `Stack`'s `enabled`.
    fn element(&self, element: &Element, ctx: &str) -> Option<String> {
        let Some((_, properties)) = COMPONENTS.iter().find(|(kind, _)| *kind == element.kind)
        else {
            // The target's capability check names the component.
            return None;
        };
        for name in element.props.keys() {
            if !properties.contains(&name.as_str()) && !COMMON.contains(&name.as_str()) {
                self.unsupported(
                    format!("`{}({name}:)`", element.kind),
                    Some(&element.source),
                );
            }
        }
        // Pictures of resources turn; anything else stays still.
        if element.expr("spin").is_some()
            && !(element.kind == "Image" && element.resource("source").is_some())
        {
            self.diagnostics.borrow_mut().push(
                Diagnostic::warning(format!(
                    "the GPUI target turns only an `Image` of a resource: this `{}` stays still",
                    element.kind
                ))
                .at(element.source.clone()),
            );
        }
        let mut scope = ctx.to_string();
        if let Some(appearance) = element.keyword("appearance") {
            scope = format!("&({scope}).with_appearance({})", appearance == "dark");
        }
        if element.kind == "Stack"
            && let Some(enabled) = element.expr("enabled")
        {
            scope = format!("&({scope}).with_enabled({})", self.condition(enabled));
        }
        // Text and icons in a styled stack take its foreground.
        if element.kind == "Stack" {
            let style = self.stack_style(element);
            if style != "None" {
                scope = format!("&({scope}).with_surface({style})");
            }
        }
        // A container whose content shows while the pointer is over it.
        if reveals(&element.children) {
            scope = format!("&({scope}).revealing()");
        }
        let code = self.element_code(element)?;
        let code = if element.prop("reveal").is_some() {
            format!("b::reveal(c, {code})")
        } else {
            code
        };
        let code = if element.prop("default_focus").is_some() {
            format!(
                "b::focus_on_appear(c, {}, {code})",
                string(element.id.as_deref().unwrap_or(&element.path))
            )
        } else {
            code
        };
        Some(format!("{{ let c = {scope}; {code} }}"))
    }

    /// An element built in context `c`.
    fn element_code(&self, element: &Element) -> Option<String> {
        let at = Some(&element.source);
        let id = || string(element.id.as_deref().unwrap_or(&element.path));
        let text = |name: &str| {
            element
                .text(name)
                .map(|text| self.text(text, at))
                .unwrap_or_else(|| "String::new()".into())
        };
        let optional_text = |name: &str| match element.text(name) {
            Some(text) => format!("Some({})", self.text(text, at)),
            None => "None".into(),
        };
        let column = |nodes: &[Node]| self.children(nodes, Axis::Column);
        let shape = || match element.keyword("shape") {
            Some("capsule") => "b::Shape::Capsule",
            Some("chip") => "b::Shape::Chip",
            _ => "b::Shape::Default",
        };
        Some(match element.kind.as_str() {
            "Column" | "Row" | "Flow" => {
                let axis = if element.kind == "Column" {
                    Axis::Column
                } else {
                    Axis::Row
                };
                format!(
                    "b::{}(c, {}, {})",
                    element.kind.to_ascii_lowercase(),
                    self.flex(element),
                    self.children(&element.children, axis)
                )
            }
            "Stack" => format!(
                "b::stack(c, b::Layers {{ under_bars: {}, align: b::Align::{}, width: {}, height: {}, style: {} }}, {})",
                element.prop("under_bars").is_some(),
                stack_align(element.keyword("align")),
                self.length(element, "width"),
                self.length(element, "height"),
                self.stack_style(element),
                column(&element.children)
            ),
            "ScrollView" => {
                let horizontal = element.keyword("axis") == Some("horizontal");
                let children = self.children(
                    &element.children,
                    if horizontal { Axis::Row } else { Axis::Column },
                );
                let snap = match element.keyword("snap") {
                    Some("start") => "Start",
                    Some("center") => "Center",
                    _ => "None",
                };
                let position = element.field("position");
                let open_at = match element.number("open_at") {
                    Some(at) => format!("Some({})", float(at)),
                    None => "None".into(),
                };
                let open_center = match element.expr("open_center") {
                    Some(center) => format!("Some(rt::as_f64(&{}) as f32)", self.expr(center, at)),
                    None => match element.number("open_center") {
                        Some(center) => format!("Some({})", float(center)),
                        None => "None".into(),
                    },
                };
                if snap == "None"
                    && position.is_none()
                    && open_at == "None"
                    && open_center == "None"
                {
                    format!(
                        "b::scroll(c, {}, {horizontal}, {children})",
                        string(&element.path)
                    )
                } else {
                    // The rows of the first lists inside are its items.
                    format!(
                        "{{ let items = b::scroll_key(c, {path}); let content = {{ let c = &c.in_scroll(Some(items)); {children} }}; b::scroll_view(c, {path}, b::ScrollProps {{ horizontal: {horizontal}, snap: b::Snap::{snap}, position: {}, open_at: {open_at}, open_center: {open_center} }}, content) }}",
                        match position {
                            Some(field) => format!(
                                "Some({})",
                                field_ref(self.session_index(&field.session), field.field)
                            ),
                            None => "None".into(),
                        },
                        path = string(&element.path),
                    )
                }
            }
            "Spacer" => "b::spacer(c)".into(),
            "Grid" => format!(
                "b::grid(c, {}, b::GridProps {{ min_width: {}, gap: {}, row_gap: {}, pad: b::Pad {{ all: {}, x: {}, y: {} }}, cell_ratio: {}, cell_extra: {} }}, {})",
                string(&element.path),
                self.number(element, "min_width", 120.0),
                self.number(element, "gap", 0.0),
                self.optional_length(element, "row_gap"),
                self.number(element, "padding", 0.0),
                self.optional_length(element, "padding_x"),
                self.optional_length(element, "padding_y"),
                match element.number("cell_ratio") {
                    Some(ratio) => format!("Some({})", float(ratio)),
                    None => "None".into(),
                },
                self.number(element, "cell_extra", 0.0),
                self.children(&element.children, Axis::Column)
            ),
            "Divider" => "b::divider(c)".into(),
            "Video" => {
                if element.keyword("controls") != Some("none") {
                    self.unsupported("a `Video` with the standard control bar".into(), at);
                }
                let (source, player) = match (element.expr("source"), element.prop("player")) {
                    (Some(source), _) => (
                        format!("Some({})", self.expr(source, at)),
                        "None".to_string(),
                    ),
                    (None, Some(Prop::Player { id })) => {
                        ("None".to_string(), format!("Some({})", string(id)))
                    }
                    _ => ("None".to_string(), "None".to_string()),
                };
                format!(
                    "b::video(c, {}, b::VideoProps {{ player: {player}, source: {source}, label: {}, fit: {}, corner: {} }})",
                    id(),
                    text("label"),
                    match element.keyword("fit") {
                        Some("cover") => 1,
                        Some("fill") => 2,
                        _ => 0,
                    },
                    self.number(element, "corner", 0.0),
                )
            }
            "FocusRegion" => format!(
                "b::focus_region(c, {}, {})",
                string(&element.path),
                column(&element.children)
            ),
            "Pinch" => {
                let Some(field) = element.field("value") else {
                    self.unsupported("a `Pinch` without a draft".into(), at);
                    return None;
                };
                format!(
                    "b::pinch(c, {}, {}, {}, {}, {}, {}, {})",
                    id(),
                    text("label"),
                    field_ref(self.session_index(&field.session), field.field),
                    float(element.number("min").unwrap_or(0.0)),
                    float(element.number("max").unwrap_or(1.0)),
                    self.action(element),
                    column(&element.children)
                )
            }
            "SplitButton" => {
                let model = self.model(element, "checked")?;
                format!(
                    "b::split_button(c, {}, {}, {model}, b::SplitProps {{ icon: {}, shape: {}, height: {}, more_label: {}, shortcut: {} }}, || {})",
                    id(),
                    text("label"),
                    self.icon(element.icon("icon")),
                    shape(),
                    self.optional_length(element, "height"),
                    text("more_label"),
                    self.shortcut(element),
                    column(&element.children)
                )
            }
            "PreviewList" => {
                let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
                    element.expr("items"),
                    &element.template,
                    element.prop("key"),
                ) else {
                    self.unsupported("this `PreviewList`".into(), at);
                    return None;
                };
                format!(
                    "b::preview_list(c, {}, rt::as_list({}), {field}, b::PreviewProps {{ item_height: {}, gap: {}, more_height: {} }}, &|c: &b::Ctx, item: V| {{ let c = &c.with_local({}, item); {} }})",
                    string(&element.path),
                    self.expr(items, at),
                    float(element.number("item_height").unwrap_or(44.0)),
                    float(element.number("gap").unwrap_or(0.0)),
                    float(element.number("more_height").unwrap_or(22.0)),
                    string(&template.var),
                    column(&element.children)
                )
            }
            "Accordion" => format!(
                "b::accordion(c, b::AccordionProps {{ selected: rt::as_f64(&{}) as usize, collapsed_width: {}, height: {}, gap: {}, corner: {} }}, {})",
                element
                    .expr("selected")
                    .map(|selected| self.expr(selected, at))
                    .unwrap_or_else(|| "V::Int(0)".into()),
                self.number(element, "collapsed_width", 80.0),
                self.optional_length(element, "height"),
                self.number(element, "gap", 0.0),
                self.number(element, "corner", 0.0),
                self.children(&element.children, Axis::Row)
            ),
            "Toolbar" => {
                // Items before the first `Spacer` stand at the leading end.
                let split = element.children.iter().position(
                    |node| matches!(node, Node::Element(child) if child.kind == "Spacer"),
                );
                let (leading, trailing) = match split {
                    Some(at) => (&element.children[..at], &element.children[at + 1..]),
                    None => (&element.children[..0], &element.children[..]),
                };
                format!(
                    "{{ let c = &c.in_toolbar(true); b::toolbar(c, {}, {}) }}",
                    self.children(leading, Axis::Row),
                    self.children(trailing, Axis::Row)
                )
            }
            "Search" => {
                let Some(field) = element.field("value") else {
                    self.unsupported("a `Search` without a draft".into(), at);
                    return None;
                };
                let number = |name: &str| match element.number(name) {
                    Some(value) => format!("Some({})", float(value)),
                    None => "None".into(),
                };
                format!(
                    "b::search(c, {}, {}, b::SearchProps {{ prompt: {}, panel_prompt: {}, palette: {}, width: {}, height: {}, font_size: {}, suggest: {}, shortcut: {} }}, {}, {})",
                    id(),
                    field_ref(self.session_index(&field.session), field.field),
                    text("prompt"),
                    optional_text("panel_prompt"),
                    element.keyword("style") == Some("palette"),
                    number("width"),
                    number("height"),
                    number("font_size"),
                    element.prop("suggest").is_some(),
                    self.shortcut(element),
                    self.action(element),
                    if element.children.is_empty() {
                        "None".to_string()
                    } else {
                        format!("Some(&|c: &b::Ctx| {})", column(&element.children))
                    }
                )
            }
            "SearchItem" => format!(
                "b::search_item(c, {}, b::SearchItemProps {{ label: {}, icon: {}, detail: {}, hint: {}, navigation: {}, pick: {} }}, {}, {})",
                id(),
                text("label"),
                self.icon(element.icon("icon")),
                optional_text("detail"),
                optional_text("hint"),
                match element.expr("navigation") {
                    Some(navigation) => self.condition(navigation),
                    None => "true".into(),
                },
                self.pick(element),
                self.action(element),
                if element.children.is_empty() {
                    "None".to_string()
                } else {
                    format!("Some({})", column(&element.children))
                }
            ),
            "Query" => {
                let (Some(binding), Some(template)) = (element.query("query"), &element.template)
                else {
                    return None;
                };
                format!(
                    "b::query(c, {}, {}, vec![{}], b::QueryProps {{ loading: {}, failed: {}, empty: {}, icon: {} }}, |c: &b::Ctx, data: V| {{ let c = &c.with_local({}, data); {} }})",
                    id(),
                    binding.query,
                    binding
                        .args
                        .iter()
                        .map(|arg| self.expr(arg, at))
                        .collect::<Vec<_>>()
                        .join(", "),
                    optional_text("loading"),
                    optional_text("failed"),
                    optional_text("empty"),
                    self.icon(element.icon("icon")),
                    string(&template.var),
                    column(&element.children)
                )
            }
            "Text" => format!(
                "b::text(c, {}, b::TextProps {{ style: b::TextStyle::{}, emphasis: b::Emphasis::{}, lines: {}, size: {}, color: {}, line_height: {}, markdown: {} }})",
                text("content"),
                pascal(element.keyword("style").unwrap_or("body")),
                pascal(element.keyword("emphasis").unwrap_or("primary")),
                match element.prop("lines") {
                    Some(Prop::Number { value }) => format!("Some({})", *value as usize),
                    Some(Prop::Expr { expr }) => {
                        format!("Some(rt::as_f64(&{}) as usize)", self.expr(expr, at))
                    }
                    _ => "None".into(),
                },
                self.optional_length(element, "font_size"),
                self.text_color(element),
                self.optional_length(element, "line_height"),
                element.prop("markdown").is_some(),
            ),
            "Label" => format!(
                "b::label(c, {}, {}, b::TextStyle::{}, {}, {})",
                text("title"),
                self.icon(element.icon("icon")),
                pascal(element.keyword("style").unwrap_or("body")),
                self.optional_length(element, "font_size"),
                element.keyword("display") == Some("icon"),
            ),
            "Image" => {
                let fit = match element.keyword("fit") {
                    Some("cover") | Some("fill_width") => "Cover",
                    Some("fill") | Some("stretch") => "Fill",
                    _ => "Contain",
                };
                let template = element.keyword("rendering") == Some("template");
                let aspect = match element.number("aspect") {
                    Some(aspect) => format!("Some({})", float(aspect)),
                    None => "None".into(),
                };
                let Some(source) = element.resource("source") else {
                    let source = element.expr("source")?;
                    return Some(format!(
                        "b::image_url(c, rt::url(&{}), {}, {}, {}, b::Fit::{fit}, {template}, {aspect})",
                        self.expr(source, at),
                        self.optional_length(element, "width"),
                        self.optional_length(element, "height"),
                        self.optional_length(element, "corner"),
                    ));
                };
                // GPUI turns pictures of resources only (`check` warns of
                // the rest).
                let spin = match element.expr("spin") {
                    Some(when) => format!(
                        "Some(({}, b::Spin {{ on: rt::truthy(&{}), period: {} }}))",
                        string(&element.path),
                        self.expr(when, at),
                        float(element.number("spin_period").unwrap_or(1.0))
                    ),
                    None => "None".into(),
                };
                format!(
                    "b::image(c, {}, {}, {}, {}, {}, b::Fit::{fit}, {template}, {aspect}, {spin})",
                    string(source),
                    match element.resource("dark_source") {
                        Some(dark) => format!("Some({})", string(dark)),
                        None => "None".into(),
                    },
                    self.optional_length(element, "width"),
                    self.optional_length(element, "height"),
                    self.optional_length(element, "corner"),
                )
            }
            "Progress" => format!(
                "b::progress(c, {}, {})",
                match element.expr("value") {
                    Some(value) => format!(
                        "Some((rt::as_f64(&{}), {}))",
                        self.expr(value, at),
                        element
                            .expr("total")
                            .map(|total| format!("rt::as_f64(&{})", self.expr(total, at)))
                            .or_else(|| element.number("total").map(|total| format!("{total:?}")))
                            .unwrap_or_else(|| "1.0".into())
                    ),
                    None => "None".into(),
                },
                optional_text("label")
            ),
            "Button" => {
                let prominence = match element.keyword("prominence").unwrap_or("standard") {
                    "prominent" => "Prominent",
                    "plain" => "Plain",
                    _ => "Standard",
                };
                let role = match element.keyword("role").unwrap_or("normal") {
                    "destructive" => "Destructive",
                    "cancel" => "Cancel",
                    _ => "Normal",
                };
                let props = format!(
                    "b::ButtonProps {{ label: {}, icon: {}, prominence: b::Prominence::{prominence}, role: b::Role::{role}, enabled: {}, shape: {}, height: {}, style: {}, pick: {}, shortcut: {} }}",
                    text("label"),
                    self.icon(element.icon("icon")),
                    self.enabled(element),
                    shape(),
                    self.optional_length(element, "height"),
                    self.control_style(element),
                    self.pick(element),
                    self.shortcut(element),
                );
                if element.children.is_empty() {
                    format!("b::button(c, {}, {props}, {})", id(), self.action(element))
                } else {
                    format!(
                        "b::button_with(c, {}, {props}, {}, &|c: &b::Ctx| {})",
                        id(),
                        self.action(element),
                        column(&element.children)
                    )
                }
            }
            "IconButton" => format!(
                "b::icon_button(c, {}, {}, b::IconButtonProps {{ label: {}, prominence: b::Prominence::{}, enabled: {}, height: {}, size: {}, style: {}, pick: {}, shortcut: {} }}, {})",
                id(),
                string(self.icon_name(element.icon("icon").unwrap_or("questionmark"))),
                text("label"),
                match element.keyword("prominence") {
                    Some("prominent") => "Prominent",
                    Some("plain") => "Plain",
                    _ => "Standard",
                },
                self.enabled(element),
                self.optional_length(element, "height"),
                self.optional_length(element, "font_size"),
                self.control_style(element),
                self.pick(element),
                self.shortcut(element),
                self.action(element)
            ),
            "Switch" => {
                let model = self.model(element, "checked")?;
                format!(
                    "b::switch(c, {}, {}, {model}, b::SwitchProps {{ style: b::SwitchStyle::{}, label_hidden: {}, icon: {}, shape: {}, height: {} }})",
                    id(),
                    text("label"),
                    match element.keyword("style") {
                        Some("checkbox") => "Checkbox",
                        Some("button") => "Button",
                        _ => "Switch",
                    },
                    element.prop("label_hidden").is_some(),
                    self.icon(element.icon("icon")),
                    shape(),
                    self.optional_length(element, "height"),
                )
            }
            "Picker" => {
                let model = self.model(element, "selected")?;
                let ty = element
                    .expr("selected")
                    .map(|expr| expr.ty.clone())
                    .or_else(|| element.field("value").map(|field| self.field_type(field)));
                let options = self.options(element, ty.as_ref())?;
                format!(
                    "b::picker(c, {}, {}, {}, {model}, {options}, b::PickerStyle::{}, {}, {})",
                    id(),
                    text("label"),
                    element.prop("label_hidden").is_some(),
                    if element.keyword("style") == Some("segmented") {
                        "Segmented"
                    } else {
                        "Menu"
                    },
                    shape(),
                    self.optional_length(element, "height"),
                )
            }
            "LanguagePicker" => format!("b::language_picker(c, {}, {})", id(), text("label")),
            "TextInput" => {
                let Some(field) = element.field("value") else {
                    self.unsupported("a `TextInput` without a draft".into(), at);
                    return None;
                };
                let kind = match element.keyword("kind").unwrap_or("plain") {
                    "secure" => "Secure",
                    "multiline" => "Multiline",
                    _ => "Text",
                };
                format!(
                    "b::text_input(c, {}, b::InputProps {{ label: {}, label_hidden: {}, prompt: {}, kind: b::InputKind::{kind}, clear_on_submit: {}, clear_key: {}, ..Default::default() }}, {}, {})",
                    id(),
                    text("label"),
                    element.prop("label_hidden").is_some(),
                    text("prompt"),
                    element.keyword("submit") == Some("clear"),
                    match element.expr("clear_key") {
                        Some(key) => format!("Some(rt::as_f64(&{}) as i64)", self.expr(key, at)),
                        None => "None".into(),
                    },
                    field_ref(self.session_index(&field.session), field.field),
                    self.action(element)
                )
            }
            "PopoverButton" => {
                let presented = if let Some(field) = element.field("presented") {
                    format!(
                        "Some(b::Model::Field({}, None))",
                        field_ref(self.session_index(&field.session), field.field)
                    )
                } else if let Some(open) = element.expr("open") {
                    format!(
                        "Some(b::Model::Controlled({}, {}))",
                        self.expr(open, at),
                        self.commit(element, open)
                    )
                } else {
                    "None".into()
                };
                format!(
                    "b::popover_button(c, {}, {}, {}, b::PopoverProps {{ presented: {presented}, dark: {}, style: {} }}, || {})",
                    id(),
                    text("label"),
                    self.icon(element.icon("icon")),
                    match element.keyword("panel_appearance") {
                        Some("dark") => "Some(true)",
                        Some("light") => "Some(false)",
                        _ => "None",
                    },
                    self.surface_style("PopoverButton", element),
                    column(&element.children)
                )
            }
            "Modal" => format!(
                "b::modal(c, b::ModalProps {{ width: {}, height: {}, cover_compact: {} }}, {})",
                self.optional_length(element, "width"),
                self.optional_length(element, "height"),
                element.keyword("cover") == Some("compact"),
                column(&element.children)
            ),
            "Form" => {
                let props = format!(
                    "b::FormProps {{ gap: {}, row_height: {}, padding_x: {}, padding_y: {}, style: {} }}",
                    self.number(element, "gap", 16.0),
                    self.optional_length(element, "row_height"),
                    self.optional_length(element, "padding_x"),
                    self.optional_length(element, "padding_y"),
                    self.surface_style("Form", element),
                );
                format!(
                    "{{ let form = {props}; b::form(c, form, {}, {{ let c = &c.in_form(form); {} }}) }}",
                    optional_text("title"),
                    column(&element.children)
                )
            }
            "Section" => format!(
                "b::section(c, {}, {}, b::SectionProps {{ plain: {}, padding_x: {}, padding_y: {} }}, {})",
                optional_text("title"),
                optional_text("footer"),
                element.keyword("style") == Some("plain"),
                self.optional_length(element, "padding_x"),
                self.optional_length(element, "padding_y"),
                column(&element.children)
            ),
            "Tabs" => self.tabs(element, "c")?,
            "Menu" => format!(
                "b::menu(c, {}, {}, b::MenuProps {{ icon: {}, icon_only: {}, shape: {}, height: {}, hide_disabled: {} }}, {})",
                id(),
                text("label"),
                self.icon(element.icon("icon")),
                element.keyword("style") == Some("icon"),
                shape(),
                self.optional_length(element, "height"),
                element.prop("hide_disabled").is_some(),
                self.entries(&element.children, "b::MenuEntry", &|out, child| {
                    match child.kind.as_str() {
                        "Divider" => out.push_str("v.push(b::MenuEntry::Divider); "),
                        "MenuItem" => {
                            let at = Some(&child.source);
                            out.push_str(&format!(
                                "v.push(b::menu_item(c, {}, {}, {}, {}, {}, {}, {}, {})); ",
                                string(child.id.as_deref().unwrap_or(&child.path)),
                                child
                                    .text("label")
                                    .map(|label| self.text(label, at))
                                    .unwrap_or_else(|| "String::new()".into()),
                                self.icon(child.icon("icon")),
                                child.keyword("role") == Some("destructive"),
                                self.enabled(child),
                                self.pick(child),
                                self.shortcut(child),
                                self.action(child)
                            ));
                        }
                        other => {
                            self.unsupported(format!("`{other}` in a `Menu`"), Some(&child.source))
                        }
                    }
                })
            ),
            "ControlGroup" => format!(
                "b::control_group(c, {}, {})",
                self.optional_length(element, "height"),
                self.children(&element.children, Axis::Row)
            ),
            "Slider" => {
                let model = self.model(element, "current")?;
                let bound =
                    |name: &str, fallback: f64| match (element.expr(name), element.number(name)) {
                        (Some(expr), _) => format!("rt::as_f64(&{})", self.expr(expr, at)),
                        (None, Some(value)) => format!("{value:?}"),
                        (None, None) => format!("{fallback:?}"),
                    };
                format!(
                    "b::slider(c, {}, {}, {model}, b::SliderProps {{ tint: {}, min: {}, max: {}, step: {}, continuous: {}, label_hidden: {}, min_icon: {}, max_icon: {} }})",
                    id(),
                    text("label"),
                    match element.keyword("tint") {
                        Some(tint) => format!("Some({})", string(tint)),
                        None => "None".into(),
                    },
                    bound("min", 0.0),
                    bound("max", 1.0),
                    match (element.expr("step"), element.number("step")) {
                        (Some(expr), _) => format!("Some(rt::as_f64(&{}))", self.expr(expr, at)),
                        (None, Some(step)) => format!("Some({step:?})"),
                        (None, None) => "None".into(),
                    },
                    element.prop("continuous").is_some(),
                    element.prop("label_hidden").is_some(),
                    self.icon(element.icon("min_icon")),
                    self.icon(element.icon("max_icon")),
                )
            }
            "Shortcut" => {
                let key = match element.prop("key") {
                    Some(Prop::Keyword { value }) => format!("{}.to_string()", string(value)),
                    Some(Prop::Expr { expr }) => format!("rt::plain(&{})", self.expr(expr, at)),
                    _ => "String::new()".into(),
                };
                format!(
                    "b::shortcut(c, {key}, {}, {})",
                    self.enabled(element),
                    self.action(element)
                )
            }
            "ShortcutInput" => {
                let Some(current) = element.expr("current") else {
                    self.unsupported("a `ShortcutInput` without `current`".into(), at);
                    return None;
                };
                format!(
                    "b::shortcut_input(c, {}, rt::plain(&{}), b::ShortcutInputProps {{ label: {}, label_hidden: {}, prompt: {}, empty: {}, enabled: {} }}, {})",
                    id(),
                    self.expr(current, at),
                    text("label"),
                    element.prop("label_hidden").is_some(),
                    text("prompt"),
                    text("empty"),
                    self.enabled(element),
                    self.commit(element, current)
                )
            }
            "Toast" => format!(
                "b::toast(c, {}, {}, {}, {})",
                text("content"),
                self.icon(element.icon("icon")),
                match element.expr("key") {
                    Some(key) => format!("Some(rt::plain(&{}))", self.expr(key, at)),
                    None => "None".into(),
                },
                match element.number("duration") {
                    Some(duration) => format!("Some({duration:?})"),
                    None => "None".into(),
                }
            ),
            "Timer" => format!(
                "b::timer(c, {}, {}, {}, {})",
                string(&element.path),
                match (element.expr("interval"), element.number("interval")) {
                    (Some(expr), _) => format!("rt::as_f64(&{})", self.expr(expr, at)),
                    (None, Some(value)) => format!("{value:?}"),
                    (None, None) => "1.0".into(),
                },
                self.enabled(element),
                self.action(element)
            ),
            "Watch" => format!(
                "b::watch(c, {}, {}, {})",
                string(&element.path),
                self.expr(element.expr("value")?, at),
                self.action(element)
            ),
            "ScaleBox" => format!(
                "b::scale_box(c, {}, {}, {}, || {})",
                string(&element.path),
                float(element.number("width").unwrap_or(0.0)),
                float(element.number("height").unwrap_or(0.0)),
                column(&element.children)
            ),
            "Positioned" => format!(
                "b::positioned(c, b::Place {{ x: {}, y: {}, width: {}, height: {}, bleed: {} }}, {})",
                self.number(element, "x", 0.0),
                self.number(element, "y", 0.0),
                self.optional_length(element, "width"),
                self.optional_length(element, "height"),
                element.prop("bleed").is_some(),
                column(&element.children)
            ),
            "Rail" => {
                let vertical = element.keyword("axis") == Some("vertical");
                let anchor = match element.keyword("anchor") {
                    Some("start") => "Start",
                    Some("end") => "End",
                    _ => "Center",
                };
                let axis = if vertical { Axis::Column } else { Axis::Row };
                let children =
                    self.entries(&element.children, "(b::El, b::Placement)", &|out, child| {
                        let fraction = |name: &str| match (child.expr(name), child.number(name)) {
                            (Some(expr), _) => Some(format!(
                                "(rt::as_f64(&{}) as f32)",
                                self.expr(expr, Some(&child.source))
                            )),
                            (None, Some(value)) => Some(float(value)),
                            (None, None) => None,
                        };
                        let placement = match (fraction("at"), fraction("from"), fraction("to")) {
                            (Some(at), _, _) => format!("b::Placement::At({at})"),
                            (None, Some(from), Some(to)) => {
                                format!("b::Placement::Span({from}, {to})")
                            }
                            (None, Some(from), None) => format!("b::Placement::Span({from}, 1.0)"),
                            (None, None, Some(to)) => format!("b::Placement::Span(0.0, {to})"),
                            _ => "b::Placement::At(0.0)".into(),
                        };
                        let ctx =
                            format!("&c.grown({})", self.element_greed(child, axis).grow(axis));
                        if let Some(code) = self.element(child, &ctx) {
                            out.push_str(&format!("v.push(({code}, {placement})); "));
                        }
                    });
                format!("b::rail(c, {vertical}, b::RailAnchor::{anchor}, {children})")
            }
            "Navigation" => {
                let Some(path) = element.field("path") else {
                    self.unsupported("a `Navigation` without a path".into(), at);
                    return None;
                };
                let session = self.session_index(&path.session);
                let field = field_ref(session, path.field);
                let destination = element.children.iter().find_map(|node| match node {
                    Node::Element(child) if child.kind == "Destination" => Some(child),
                    _ => None,
                });
                let root: Vec<Node> = element
                    .children
                    .iter()
                    .filter(
                        |node| !matches!(node, Node::Element(child) if child.kind == "Destination"),
                    )
                    .cloned()
                    .collect();
                let var = destination
                    .and_then(|destination| destination.template.as_ref())
                    .map(|template| template.var.clone())
                    .unwrap_or_else(|| "route".into());
                let pushed = destination
                    .map(|destination| column(&destination.children))
                    .unwrap_or_else(|| "Vec::new()".into());
                format!(
                    "{{ let c = &c.in_navigation({field}); let path = rt::as_list(c.session({session}, {})); match path.last().cloned() {{ Some(route) => {{ let c = &c.with_local({}, route); b::navigation(c, {field}, path.len(), None, {pushed}) }} None => b::navigation(c, {field}, 0, None, {}) }} }}",
                    path.field,
                    string(&var),
                    column(&root)
                )
            }
            "NavLink" => format!(
                "b::nav_link(c, {}, {}, {}, {})",
                id(),
                self.row_label(&element.children)
                    .unwrap_or_else(|| "String::new()".into()),
                element
                    .expr("route")
                    .map(|route| self.expr(route, at))
                    .unwrap_or_else(|| "V::Unit".into()),
                column(&element.children)
            ),
            "SplitView" => {
                let sidebar = element.children.iter().find_map(|node| match node {
                    Node::Element(child) if child.kind == "Sidebar" => Some(child),
                    _ => None,
                });
                let detail = element.children.iter().find_map(|node| match node {
                    Node::Element(child) if child.kind == "Detail" => Some(child),
                    _ => None,
                });
                let (Some(sidebar), Some(detail)) = (sidebar, detail) else {
                    self.unsupported("a `SplitView` without a sidebar and a detail".into(), at);
                    return None;
                };
                let Some(selection) = sidebar.field("selection") else {
                    self.unsupported(
                        "a `Sidebar` without a selection".into(),
                        Some(&sidebar.source),
                    );
                    return None;
                };
                let session = self.session_index(&selection.session);
                let selection_ref = field_ref(session, selection.field);
                let part = |kind: &str| -> Vec<Node> {
                    sidebar
                        .children
                        .iter()
                        .filter_map(|node| match node {
                            Node::Element(child) if child.kind == kind => {
                                Some(child.children.clone())
                            }
                            _ => None,
                        })
                        .flatten()
                        .collect()
                };
                let items: Vec<Node> = sidebar
                    .children
                    .iter()
                    .filter(|node| {
                        !matches!(node, Node::Element(child) if matches!(child.kind.as_str(), "SidebarHeader" | "SidebarFooter"))
                    })
                    .cloned()
                    .collect();
                // The title of the detail: the chosen item's label.
                let mut labels = Vec::new();
                sidebar.children.iter().for_each(|node| {
                    node.walk(&mut |node| {
                        if let Node::Element(item) = node
                            && item.kind == "SidebarItem"
                            && let (Some(value), Some(label)) =
                                (item.expr("value"), item.text("label"))
                        {
                            labels.push(format!(
                                "({}, {})",
                                self.expr(value, Some(&item.source)),
                                self.text(label, Some(&item.source))
                            ));
                        }
                    })
                });
                let title = if detail
                    .prop("titled")
                    .and_then(|_| detail.expr("titled"))
                    .is_some_and(|expr| {
                        matches!(
                            &expr.kind,
                            ExprKind::Literal {
                                value: Value::Bool(false)
                            }
                        )
                    }) {
                    "None".to_string()
                } else {
                    format!(
                        "b::sidebar_title(&c.session({session}, {}), vec![{}])",
                        selection.field,
                        labels.join(", ")
                    )
                };
                format!(
                    "b::split_view(c, {{ let c = &c.in_sidebar({selection_ref}); b::sidebar(c, {}, {}, {}, {}, {}) }}, b::detail(c, {title}, {}, {}))",
                    self.optional_length(sidebar, "width"),
                    column(&part("SidebarHeader")),
                    column(&items),
                    column(&part("SidebarFooter")),
                    self.surface_style("Sidebar", sidebar),
                    column(&detail.children),
                    self.surface_style("Detail", detail),
                )
            }
            "SidebarItem" => format!(
                "b::sidebar_item(c, {}, {}, {}, {}, {})",
                element
                    .expr("value")
                    .map(|value| self.expr(value, at))
                    .unwrap_or_else(|| "V::Unit".into()),
                text("label"),
                self.icon(element.icon("icon")),
                match element.expr("badge") {
                    Some(badge) => format!("Some(rt::as_f64(&{}) as i64)", self.expr(badge, at)),
                    None => "None".into(),
                },
                self.control_style(element),
            ),
            "Canvas" => format!(
                "b::canvas(c, {}, {}, vec![{}])",
                self.length(element, "width"),
                self.length(element, "height"),
                self.canvas_ops(element).join(", ")
            ),
            "Drop" => {
                let Some(field) = element.field("value") else {
                    self.unsupported("a `Drop` without a draft".into(), at);
                    return None;
                };
                format!(
                    "b::drop_area(c, {}, {}, {}, {}, {})",
                    id(),
                    field_ref(self.session_index(&field.session), field.field),
                    match element.field("over") {
                        Some(over) => format!(
                            "Some({})",
                            field_ref(self.session_index(&over.session), over.field)
                        ),
                        None => "None".into(),
                    },
                    self.action(element),
                    column(&element.children)
                )
            }
            "Adaptive" => {
                let layouts: Vec<String> = element
                    .children
                    .iter()
                    .filter_map(|node| match node {
                        Node::Element(layout) if layout.kind == "Layout" => {
                            let size = match layout.keyword("size") {
                                // A desktop is never a television.
                                Some("tv") => return None,
                                Some("compact") => "Compact",
                                Some("medium") => "Medium",
                                Some("large") => "Large",
                                Some("expanded") => "Expanded",
                                other => {
                                    self.unsupported(
                                        format!("`Layout(size: {})`", other.unwrap_or("")),
                                        Some(&layout.source),
                                    );
                                    return None;
                                }
                            };
                            Some(format!(
                                "(b::Size::{size}, Box::new(|| {}) as Box<dyn FnOnce() -> Vec<b::El> + '_>)",
                                column(&layout.children)
                            ))
                        }
                        _ => None,
                    })
                    .collect();
                format!("b::adaptive(c, vec![{}])", layouts.join(", "))
            }
            _ => return None,
        })
    }

    /// The text a row shows first, the accessible name of a link.
    fn row_label(&self, nodes: &[Node]) -> Option<String> {
        nodes.iter().find_map(|node| match node {
            Node::Element(element) => match element.kind.as_str() {
                "Text" => element.text("content").map(|text| self.text(text, None)),
                "Label" => element.text("title").map(|text| self.text(text, None)),
                _ => self.row_label(&element.children),
            },
            _ => None,
        })
    }

    /// A list of `item` built from `nodes` through control flow: `on`
    /// pushes what each element gives onto `v`.
    fn entries(&self, nodes: &[Node], item: &str, on: &dyn Fn(&mut String, &Element)) -> String {
        let mut body = String::new();
        for node in nodes {
            self.flow(&mut body, node, on);
        }
        format!("{{ let mut v: Vec<{item}> = Vec::new(); {body} v }}")
    }

    fn flow(&self, out: &mut String, node: &Node, on: &dyn Fn(&mut String, &Element)) {
        match node {
            Node::Element(element) if element.kind == "ForEach" => {
                let (Some(items), Some(template), Some(Prop::Key { field, .. })) = (
                    element.expr("items"),
                    &element.template,
                    element.prop("key"),
                ) else {
                    return;
                };
                out.push_str(&format!(
                    "for item in rt::as_list({}) {{ let key = rt::key_text(&rt::at(item.clone(), &[{field}])); let c = &c.with_local({}, item).with_key(key); ",
                    self.expr(items, Some(&element.source)),
                    string(&template.var)
                ));
                for node in &element.children {
                    self.flow(out, node, on);
                }
                out.push_str("} ");
            }
            Node::Element(element) => {
                for name in element.props.keys() {
                    let known = COMPONENTS
                        .iter()
                        .find(|(kind, _)| *kind == element.kind)
                        .is_some_and(|(_, properties)| properties.contains(&name.as_str()));
                    if !known && !COMMON.contains(&name.as_str()) {
                        self.unsupported(
                            format!("`{}({name}:)`", element.kind),
                            Some(&element.source),
                        );
                    }
                }
                on(out, element)
            }
            Node::Component(reference) => self.unsupported(
                format!("the component `{}` here", reference.component),
                Some(&reference.source),
            ),
            Node::If(node) => {
                for (index, (condition, body)) in node.branches.iter().enumerate() {
                    if index > 0 {
                        out.push_str("else ");
                    }
                    out.push_str(&format!("if {} {{ ", self.condition(condition)));
                    for node in body {
                        self.flow(out, node, on);
                    }
                    out.push_str("} ");
                }
                if !node.otherwise.is_empty() {
                    out.push_str("else { ");
                    for node in &node.otherwise {
                        self.flow(out, node, on);
                    }
                    out.push_str("} ");
                }
            }
            Node::IfLet(node) => {
                out.push_str(&format!(
                    "if let Some(x) = rt::some({}) {{ let c = &c.with_local({}, x); ",
                    self.expr(&node.value, None),
                    string(&node.var)
                ));
                for node in &node.then {
                    self.flow(out, node, on);
                }
                out.push_str("} ");
                if !node.otherwise.is_empty() {
                    out.push_str("else { ");
                    for node in &node.otherwise {
                        self.flow(out, node, on);
                    }
                    out.push_str("} ");
                }
            }
            Node::Match(node) => {
                out.push_str(&format!(
                    "{{ let m = {}; match rt::tag(&m) {{ ",
                    self.expr(&node.value, None)
                ));
                let mut wildcard = false;
                for arm in &node.arms {
                    match arm.variant {
                        Some(variant) => out.push_str(&format!("Some({variant}) => {{ ")),
                        None => {
                            wildcard = true;
                            out.push_str("_ => { ");
                        }
                    }
                    for binding in &arm.bindings {
                        out.push_str(&format!(
                            "let c = &c.with_local({}, rt::payload(m.clone(), {})); ",
                            string(&binding.name),
                            binding.field
                        ));
                    }
                    for node in &arm.body {
                        self.flow(out, node, on);
                    }
                    out.push_str("} ");
                }
                if !wildcard {
                    out.push_str("_ => {} ");
                }
                out.push_str("} } ");
            }
        }
    }

    /// The drawing elements of a `Canvas`, in order.
    fn canvas_ops(&self, element: &Element) -> Vec<String> {
        let role = |child: &Element, name: &str| match child.keyword(name) {
            Some(role) => format!("Some(b::Color::Role({}))", string(role)),
            None => "None".into(),
        };
        let mut ops = Vec::new();
        for node in &element.children {
            let Node::Element(child) = node else {
                self.unsupported("control flow in a `Canvas`".into(), Some(&element.source));
                continue;
            };
            for name in child.props.keys() {
                let known = COMPONENTS
                    .iter()
                    .find(|(kind, _)| *kind == child.kind)
                    .is_some_and(|(_, properties)| properties.contains(&name.as_str()));
                if !known && !COMMON.contains(&name.as_str()) {
                    self.unsupported(format!("`{}({name}:)`", child.kind), Some(&child.source));
                }
            }
            let number = |name: &str, fallback: f64| float(child.number(name).unwrap_or(fallback));
            let optional = |name: &str| match child.number(name) {
                Some(value) => format!("Some({})", float(value)),
                None => "None".into(),
            };
            let stroke = |child: &Element| match child.keyword("stroke") {
                Some(stroke) => format!(
                    "Some((b::Color::Role({}), {}))",
                    string(stroke),
                    float(child.number("stroke_width").unwrap_or(1.0))
                ),
                None => "None".into(),
            };
            match child.kind.as_str() {
                "Shape" => {
                    let kind = match child.keyword("kind").unwrap_or("rect") {
                        "rounded" => "Rounded",
                        "capsule" => "Capsule",
                        "ellipse" => "Ellipse",
                        _ => "Rect",
                    };
                    let fade = match child.keyword("fade") {
                        Some("up") => "Some(b::Fade::Up)",
                        Some("down") => "Some(b::Fade::Down)",
                        Some("left") => "Some(b::Fade::Left)",
                        Some("right") => "Some(b::Fade::Right)",
                        _ => "None",
                    };
                    let dash = match child.number("dash") {
                        Some(dash) => format!(
                            "Some(({}, {}))",
                            float(dash),
                            float(child.number("dash_gap").unwrap_or(dash))
                        ),
                        None => "None".into(),
                    };
                    ops.push(format!(
                        "b::Op::Shape(b::ShapeOp {{ kind: b::ShapeKind::{kind}, x: {}, y: {}, width: {}, height: {}, corner: {}, fill: {}, picture: {}, fade: {fade}, stroke: {}, dash: {dash}, opacity: {} }})",
                        number("x", 0.0),
                        number("y", 0.0),
                        optional("width"),
                        optional("height"),
                        number("corner", 0.0),
                        role(child, "fill"),
                        match child.expr("fill_from") {
                            Some(picture) => format!("Some(rt::url(&{}))", self.expr(picture, Some(&child.source))),
                            None => "None".into(),
                        },
                        stroke(child),
                        number("opacity", 1.0),
                    ));
                }
                "Series" => {
                    let values = child
                        .expr("values")
                        .map(|list| {
                            format!(
                                "rt::as_list({}).iter().map(rt::as_f64).collect()",
                                self.expr(list, Some(&child.source))
                            )
                        })
                        .unwrap_or_else(|| "Vec::new()".into());
                    let max = match (child.expr("max"), child.number("max")) {
                        (Some(max), _) => {
                            format!("Some(rt::as_f64(&{}))", self.expr(max, Some(&child.source)))
                        }
                        (None, Some(max)) => format!("Some({max:?})"),
                        (None, None) => "None".into(),
                    };
                    ops.push(format!(
                        "b::Op::Series(b::SeriesOp {{ values: {values}, area: {}, max: {max}, fill: {}, stroke: {}, opacity: {} }})",
                        child.keyword("kind") == Some("area"),
                        role(child, "fill"),
                        stroke(child),
                        number("opacity", 1.0),
                    ));
                }
                "Pattern" => {
                    let Some(pattern) = child
                        .keyword("name")
                        .and_then(|name| self.app.style.pattern(name))
                    else {
                        continue;
                    };
                    let bands: Vec<String> = pattern
                        .bands
                        .iter()
                        .map(|band| {
                            format!(
                                "b::Band {{ width: {}, color: {}, opacity: {} }}",
                                float(band.width),
                                match &band.color {
                                    Some(color) => format!("Some({})", color_expr(color)),
                                    None => "None".into(),
                                },
                                float(band.opacity)
                            )
                        })
                        .collect();
                    ops.push(format!(
                        "b::Op::Pattern(b::PatternOp {{ bands: &[{}], twill: {}, opacity: {} }})",
                        bands.join(", "),
                        pattern.twill,
                        number("opacity", 1.0),
                    ));
                }
                _ => {}
            }
        }
        ops
    }

    fn tabs(&self, element: &Element, c: &str) -> Option<String> {
        let at = Some(&element.source);
        let Some(field) = element.field("selection") else {
            self.unsupported("`Tabs` without a draft".into(), at);
            return None;
        };
        let session = self.session_index(&field.session);
        let target = field_ref(session, field.field);
        // The tabs, through any control flow around them.
        let tabs = self.entries(
            &element.children,
            "(V, String, Option<&'static str>, bool)",
            &|out, tab| {
                if tab.kind == "TabAccessory" {
                    return;
                }
                if tab.kind != "Tab" {
                    self.unsupported(format!("`{}` among `Tab`s", tab.kind), Some(&tab.source));
                    return;
                }
                let Some(value) = tab.expr("value") else {
                    return;
                };
                out.push_str(&format!(
                    "v.push(({}, {}, {}, {})); ",
                    self.expr(value, Some(&tab.source)),
                    tab.text("label")
                        .map(|label| self.text(label, Some(&tab.source)))
                        .unwrap_or_else(|| "String::new()".into()),
                    self.icon(tab.icon("icon")),
                    tab.keyword("display") == Some("icon"),
                ));
            },
        );
        // The bar kept with the tabs, while it is shown.
        let accessory = element
            .children
            .iter()
            .find_map(|node| match node {
                Node::Element(accessory) if accessory.kind == "TabAccessory" => Some(accessory),
                _ => None,
            })
            .map(|accessory| {
                format!(
                    "if {} {{ Some({}) }} else {{ None }}",
                    accessory
                        .expr("shown")
                        .map(|shown| self.condition(shown))
                        .unwrap_or_else(|| "true".into()),
                    self.children(&accessory.children, Axis::Column)
                )
            })
            .unwrap_or_else(|| "None".into());
        // The content of the chosen one.
        let content = self.entries(&element.children, "b::El", &|out, tab| {
            if tab.kind != "Tab" {
                return;
            }
            let Some(value) = tab.expr("value") else {
                return;
            };
            out.push_str(&format!(
                "if selected == {} {{ v.extend({}); }} ",
                self.expr(value, Some(&tab.source)),
                self.children(&tab.children, Axis::Column)
            ));
        });
        Some(format!(
            "{{ let selected = c.session({session}, {}); b::tabs({c}, {}, {tabs}, selected.clone(), b::Model::Field({target}, None), {content}, {accessory}) }}",
            field.field,
            string(element.id.as_deref().unwrap_or(&element.path)),
        ))
    }

    // MARK: - Properties

    fn flex(&self, element: &Element) -> String {
        let fallback = if element.kind == "Row" {
            "center"
        } else {
            "leading"
        };
        let align = match element.keyword("align").unwrap_or(fallback) {
            "leading" | "top" => "Start",
            "center" => "Center",
            "trailing" | "bottom" => "End",
            "baseline" => "Baseline",
            other => {
                self.unsupported(
                    format!("`{}(align: {other})`", element.kind),
                    Some(&element.source),
                );
                "Start"
            }
        };
        // `style!` lengths for this platform are already written in place
        // of the element's own (`bern_target_api::style::for_platforms`).
        format!(
            "b::Flex {{ gap: {}, pad: b::Pad {{ all: {}, x: {}, y: {} }}, width: {}, height: {}, align: b::Align::{align} }}",
            self.number(element, "gap", 0.0),
            self.number(element, "padding", 0.0),
            self.optional_length(element, "padding_x"),
            self.optional_length(element, "padding_y"),
            self.length(element, "width"),
            self.length(element, "height"),
        )
    }

    /// A length in points: a number, or one bound to an expression.
    fn length_value(&self, element: &Element, name: &str) -> Option<String> {
        match element.prop(name)? {
            Prop::Number { value } => Some(float(*value)),
            Prop::Expr { expr } => Some(format!(
                "(rt::as_f64(&{}) as f32)",
                self.expr(expr, Some(&element.source))
            )),
            Prop::Keyword { value } => {
                if !(element.kind == "Button" && name == "width" && value == "fill") {
                    self.unsupported(
                        format!("`{}({name}: {value})`", element.kind),
                        Some(&element.source),
                    );
                }
                None
            }
            _ => None,
        }
    }

    fn number(&self, element: &Element, name: &str, fallback: f64) -> String {
        self.length_value(element, name)
            .unwrap_or_else(|| float(fallback))
    }

    fn optional_length(&self, element: &Element, name: &str) -> String {
        match self.length_value(element, name) {
            Some(value) => format!("Some({value})"),
            None => "None".into(),
        }
    }

    fn length(&self, element: &Element, name: &str) -> String {
        match self.length_value(element, name) {
            Some(value) => format!("b::Len::Fixed({value})"),
            None => "b::Len::Auto".into(),
        }
    }

    fn text_color(&self, element: &Element) -> String {
        match element.keyword("color") {
            Some(color) => format!("Some({})", string(color)),
            None => "None".into(),
        }
    }

    fn enabled(&self, element: &Element) -> String {
        match element.expr("enabled") {
            Some(expr) => self.condition(expr),
            None => "true".into(),
        }
    }

    fn icon_name<'n>(&self, name: &'n str) -> &'n str {
        self.icons.borrow_mut().insert(name.to_string());
        name
    }

    fn icon(&self, name: Option<&str>) -> String {
        match name {
            Some(name) => format!("Some({})", string(self.icon_name(name))),
            None => "None".into(),
        }
    }

    fn pick(&self, element: &Element) -> String {
        match element.field("pick") {
            Some(field) => format!(
                "Some(({}, {}))",
                field_ref(self.session_index(&field.session), field.field),
                string(element.keyword("files").unwrap_or("any"))
            ),
            None => "None".into(),
        }
    }

    fn session_index(&self, session: &str) -> usize {
        self.app
            .sessions
            .iter()
            .position(|candidate| candidate.name == session)
            .unwrap_or(0)
    }

    fn field_type(&self, field: &bern_ir::FieldRef) -> Type {
        self.app
            .session(&field.session)
            .and_then(|session| session.fields.get(field.field))
            .map(|field| field.ty.clone())
            .unwrap_or(Type::Unit)
    }

    /// The options of a picker: the variants of its enum, with their
    /// labels.
    fn options(&self, element: &Element, ty: Option<&Type>) -> Option<String> {
        // Explicit options, with labels that may follow state.
        if !element.children.is_empty() {
            return Some(
                self.entries(&element.children, "(V, String)", &|out, child| {
                    if child.kind != "PickerOption" {
                        self.unsupported(
                            format!("`{}` in a `Picker`", child.kind),
                            Some(&child.source),
                        );
                        return;
                    }
                    let at = Some(&child.source);
                    let value = child
                        .expr("value")
                        .map(|value| self.expr(value, at))
                        .unwrap_or_else(|| "V::Unit".into());
                    let label = child
                        .text("label")
                        .map(|label| self.text(label, at))
                        .unwrap_or_else(|| "String::new()".into());
                    out.push_str(&format!("v.push(({value}, {label})); "));
                }),
            );
        }
        let Some(Type::Enum { name }) = ty else {
            self.unsupported(
                "a `Picker` of anything but an enum".into(),
                Some(&element.source),
            );
            return None;
        };
        let def = self.app.types.enum_def(name)?;
        if !def.is_unit_only() {
            self.unsupported(
                format!("a `Picker` of `{name}`, whose variants carry data"),
                Some(&element.source),
            );
            return None;
        }
        let options: Vec<String> = (0..def.variants.len())
            .map(|index| {
                format!(
                    "(rt::variant({index}, vec![]), b::text::label({}, {index}))",
                    string(name)
                )
            })
            .collect();
        Some(format!("vec![{}]", options.join(", ")))
    }

    /// What a control shows and changes: its draft field, after which its
    /// action runs; or the value `controlled` reads, which a change commits
    /// by running the action's command with the new value in its place.
    fn model(&self, element: &Element, controlled: &str) -> Option<String> {
        let at = Some(&element.source);
        if let Some(expr) = element.expr(controlled) {
            return Some(format!(
                "b::Model::Controlled({}, {})",
                self.expr(expr, at),
                self.commit(element, expr)
            ));
        }
        let Some(field) = element.field("value") else {
            self.unsupported(format!("a `{}` without a value", element.kind), at);
            return None;
        };
        Some(format!(
            "b::Model::Field({}, {})",
            field_ref(self.session_index(&field.session), field.field),
            self.action(element)
        ))
    }

    /// What a change of a value the engine holds runs: the action's
    /// command, with the new value in place of `controlled`.
    fn commit(&self, element: &Element, controlled: &Expr) -> String {
        let at = Some(&element.source);
        let commit = match element.action.as_ref() {
            Some(Action::Engine { command, args }) => {
                let values: Vec<_> = args
                    .iter()
                    .map(|arg| {
                        if arg != controlled {
                            self.expr(arg, at)
                        } else if arg.ty == Type::Int {
                            "V::Int(rt::as_f64(&v).round() as i64)".to_string()
                        } else {
                            "v.clone()".to_string()
                        }
                    })
                    .collect();
                format!("b::dispatch(c, {command}, vec![{}])", values.join(", "))
            }
            Some(other) => self.action_body(other, at),
            None => String::new(),
        };
        format!("std::rc::Rc::new(|c: &b::Ctx, v: V| {{ {commit}; }})")
    }

    // MARK: - Expressions, texts and actions

    fn binding(&self, binding: &Binding, at: Option<&SourceLocation>) -> String {
        let path = |expr: String, path: &[usize]| {
            if path.is_empty() {
                expr
            } else {
                let indices: Vec<String> = path.iter().map(|index| index.to_string()).collect();
                format!("rt::at({expr}, &[{}])", indices.join(", "))
            }
        };
        match binding {
            Binding::Engine { field, path: p } => path(format!("c.engine({field})"), p),
            Binding::Session {
                session,
                field,
                path: p,
            } => path(
                format!("c.session({}, {field})", self.session_index(session)),
                p,
            ),
            Binding::Local { name, path: p, .. } => path(format!("c.local({})", string(name)), p),
            Binding::Window { field } => {
                if !matches!(
                    field.as_str(),
                    "compact"
                        | "medium"
                        | "expanded"
                        | "large"
                        | "short"
                        | "keyboard"
                        | "tv"
                        | "idle"
                        | "fullscreen"
                ) {
                    self.unsupported(format!("`window.{field}`"), at);
                }
                format!("c.window({})", string(field))
            }
        }
    }

    fn expr(&self, expr: &Expr, at: Option<&SourceLocation>) -> String {
        match &expr.kind {
            ExprKind::Literal { value } => value_expr(value),
            ExprKind::Corner { radius, .. } => value_expr(&Value::Float(*radius)),
            ExprKind::Binding { binding } => self.binding(binding, at),
            ExprKind::Variant { variant, fields } => {
                let values: Vec<_> = fields.iter().map(|field| self.expr(field, at)).collect();
                format!("rt::variant({variant}, vec![{}])", values.join(", "))
            }
            ExprKind::If {
                cond,
                then,
                otherwise,
            } => format!(
                "(if {} {{ {} }} else {{ {} }})",
                self.condition(cond),
                self.expr(then, at),
                self.expr(otherwise, at)
            ),
            ExprKind::Not { expr } => format!("rt::not({})", self.expr(expr, at)),
            ExprKind::Neg { expr } => format!("rt::neg({})", self.expr(expr, at)),
            ExprKind::Binary { op, lhs, rhs } => {
                let (left, right) = (self.expr(lhs, at), self.expr(rhs, at));
                match op {
                    BinaryOp::And => format!("rt::and({left}, || {right})"),
                    BinaryOp::Or => format!("rt::or({left}, || {right})"),
                    other => {
                        let name = match other {
                            BinaryOp::Eq => "eq",
                            BinaryOp::Ne => "ne",
                            BinaryOp::Lt => "lt",
                            BinaryOp::Le => "le",
                            BinaryOp::Gt => "gt",
                            _ => "ge",
                        };
                        format!("rt::{name}({left}, {right})")
                    }
                }
            }
        }
    }

    fn condition(&self, expr: &Expr) -> String {
        format!("rt::truthy(&{})", self.expr(expr, None))
    }

    /// A `tr!` argument: numbers keep their value for plural branches,
    /// dates stay dates, enums are their labels.
    fn argument(&self, expr: &Expr, at: Option<&SourceLocation>) -> String {
        let value = self.expr(expr, at);
        match &expr.ty {
            Type::Record { name } if name == bern_protocol::format::CALENDAR_DATE => {
                format!("b::text::date(&{value})")
            }
            Type::Enum { name } => format!("b::text::arg(&{value}, Some({}))", string(name)),
            _ => format!("b::text::arg(&{value}, None)"),
        }
    }

    fn text(&self, text: &Text, at: Option<&SourceLocation>) -> String {
        match text {
            Text::Localized { text, args } => {
                if text.params.is_empty() {
                    format!("b::text::tr({})", string(&text.key))
                } else {
                    let values: Vec<_> = text
                        .params
                        .iter()
                        .map(|param| {
                            let value = args
                                .iter()
                                .find(|(name, _)| name == param)
                                .map(|(_, expr)| self.argument(expr, at))
                                .unwrap_or_else(|| "b::text::arg(&V::Unit, None)".into());
                            format!("({}, {value})", string(param))
                        })
                        .collect();
                    format!(
                        "b::text::fmt({}, &[{}])",
                        string(&text.key),
                        values.join(", ")
                    )
                }
            }
            Text::Dynamic { expr } => {
                let value = self.expr(expr, at);
                match &expr.ty {
                    Type::Enum { name } => {
                        format!("b::text::display(&{value}, Some({}))", string(name))
                    }
                    _ => format!("rt::plain(&{value})"),
                }
            }
        }
    }

    fn action_body(&self, action: &Action, at: Option<&SourceLocation>) -> String {
        match action {
            Action::Engine { command, args } => {
                let values: Vec<_> = args.iter().map(|arg| self.expr(arg, at)).collect();
                format!("b::dispatch(c, {command}, vec![{}])", values.join(", "))
            }
            Action::Session { field, op } => {
                let target = field_ref(self.session_index(&field.session), field.field);
                match op {
                    SessionOp::Set { value } => {
                        format!("b::session_set(c, {target}, {})", self.expr(value, at))
                    }
                    SessionOp::Toggle => format!("b::session_toggle(c, {target})"),
                    SessionOp::PushStr { value } => {
                        format!("b::session_push_str(c, {target}, {})", self.expr(value, at))
                    }
                    SessionOp::Pop => format!("b::session_pop(c, {target})"),
                }
            }
            Action::Window { op, window } => format!(
                "b::window_command(c, b::WindowOp::{}, {})",
                match op {
                    bern_ir::WindowOp::Open | bern_ir::WindowOp::Activate => "Open",
                    bern_ir::WindowOp::Close => "Close",
                    bern_ir::WindowOp::ToggleFullscreen => "ToggleFullscreen",
                },
                string(window)
            ),
            Action::Media { player, op, args } => format!(
                "b::media(c, b::MediaOp::{}, {}, vec![{}])",
                match op {
                    bern_ir::MediaOp::Open => "Open",
                    bern_ir::MediaOp::Play => "Play",
                    bern_ir::MediaOp::Pause => "Pause",
                    bern_ir::MediaOp::Seek => "Seek",
                    bern_ir::MediaOp::SetVolume => "SetVolume",
                    bern_ir::MediaOp::SetMuted => "SetMuted",
                },
                string(player),
                args.iter()
                    .map(|arg| self.expr(arg, at))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// An element's action, then the draft it clears.
    fn action(&self, element: &Element) -> String {
        let at = Some(&element.source);
        match &element.action {
            Some(action) => {
                let clear = element
                    .clearing()
                    .map(|clear| format!("{};", self.action_body(&clear, at)))
                    .unwrap_or_default();
                format!(
                    "Some(b::act(|c: &b::Ctx| {{ {}; {clear} }}))",
                    self.action_body(action, at)
                )
            }
            None => "None".into(),
        }
    }
}

fn field_ref(session: usize, field: usize) -> String {
    format!("b::Field {{ session: {session}, field: {field} }}")
}

/// One state's look from its effects; what the target cannot show is
/// left to the shared check.
fn look(effects: &[StyleEffect]) -> String {
    let mut fields = Vec::new();
    for effect in effects {
        match effect {
            StyleEffect::Background { color } => {
                fields.push(format!("background: Some({})", color_expr(color)))
            }
            StyleEffect::Foreground { color } => {
                fields.push(format!("foreground: Some({})", color_expr(color)))
            }
            StyleEffect::Border { color, width } => fields.push(format!(
                "border: Some(({}, {}))",
                color_expr(color),
                float(*width)
            )),
            StyleEffect::Shadow { radius, y, opacity } => fields.push(format!(
                "shadow: Some(({}, {}, {}))",
                float(*radius),
                float(*y),
                float(*opacity)
            )),
            StyleEffect::Opacity { value, .. } => {
                fields.push(format!("opacity: Some({})", float(*value)))
            }
            StyleEffect::Fill { color } => {
                fields.push(format!("fill: Some({})", color_expr(color)))
            }
            StyleEffect::Scale { x, y, .. } => {
                fields.push(format!("scale: Some(({}, {}))", float(*x), float(*y)))
            }
            StyleEffect::Nothing => fields.push("none: true".into()),
            _ => {}
        }
    }
    fields.push("..b::Look::NONE".into());
    format!("b::Look {{ {} }}", fields.join(", "))
}

/// Whether an element among `nodes`, through conditions and lists, shows
/// only while the pointer is over its container (`reveal: hover`).
fn reveals(nodes: &[Node]) -> bool {
    nodes.iter().any(|node| match node {
        Node::Element(element) if element.kind == "ForEach" => reveals(&element.children),
        Node::Element(element) => element.prop("reveal").is_some(),
        Node::If(node) => {
            node.branches.iter().any(|(_, body)| reveals(body)) || reveals(&node.otherwise)
        }
        Node::IfLet(node) => reveals(&node.then) || reveals(&node.otherwise),
        Node::Match(node) => node.arms.iter().any(|arm| reveals(&arm.body)),
        Node::Component(_) => false,
    })
}

fn color_expr(color: &ColorRef) -> String {
    match color {
        ColorRef::Role { role } => format!("b::Color::Role({})", string(role)),
        ColorRef::Fixed { light, dark } => format!(
            "b::Color::Fixed {{ light: ({}, {}, {}), dark: ({}, {}, {}) }}",
            light.red, light.green, light.blue, dark.red, dark.green, dark.blue
        ),
    }
}

fn stack_align(align: Option<&str>) -> &'static str {
    match align.unwrap_or("center") {
        "top_leading" => "TopLeading",
        "top" => "Top",
        "top_trailing" => "TopTrailing",
        "leading" => "Leading",
        "trailing" => "Trailing",
        "bottom_leading" => "BottomLeading",
        "bottom" => "Bottom",
        "bottom_trailing" => "BottomTrailing",
        _ => "Center",
    }
}

/// `large_title` → `LargeTitle`.
fn pascal(keyword: &str) -> String {
    keyword
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::pascal;

    #[test]
    fn keywords_become_variants() {
        assert_eq!(pascal("large_title"), "LargeTitle");
        assert_eq!(pascal("title2"), "Title2");
        assert_eq!(pascal("prominent"), "Prominent");
    }
}
