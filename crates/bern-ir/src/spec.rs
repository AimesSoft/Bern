//! Built-in component specifications.
//!
//! This table is the single definition of which components exist, which
//! properties they accept and how their children and actions are shaped.
//! The macros, the compiler and every target read it, so a component is
//! described once and never inferred from one backend's behaviour.

/// How a property value is written and checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    /// Stable control identity: an identifier or a text literal.
    Id,
    /// One of a fixed set of keywords.
    Keyword(&'static [&'static str]),
    /// `tr!` text or a displayable binding (text, int, float, labelled enum).
    DisplayText,
    /// Only `tr!` text; used where data would make no sense (window titles).
    LocalizedText,
    /// Icon name literal from the Bern icon set (SF Symbols naming).
    Icon,
    /// Name literal of a declared image resource (PNG or JPEG), such as
    /// `"images/cover.png"`.
    ImageResource,
    /// A bundled SVG document, with CSS/SMIL and document-local scripts.
    SvgResource,
    /// Numeric literal in logical units.
    Number,
    /// A keyboard shortcut: a literal such as `"cmd+k"`, `"space"` or
    /// `"cmd+shift+p"`, checked when the application is built (see
    /// [`parse_shortcut`]), or a text the shortcut follows while the
    /// application runs, such as `app.keys.open`, read by the same rules;
    /// a text that is empty or no shortcut gives none.
    Shortcut,
    /// A length in logical units: a literal, or a numeric expression the
    /// length follows, such as `if window.idle { 0 } else { 24 }`.
    Length,
    /// The kinds of file a control accepts, a literal such as
    /// `"video .ass .srt"`; see [`parse_file_kinds`].
    FileKinds,
    /// Writable session field holding a `MediaSource` (or an optional
    /// one): the file the user chose or dropped.
    FileField,
    /// Writable session `i64` or `String` field holding the key of the
    /// item a scroll view stands at.
    PositionField,
    /// A literal `true` or `false`, fixed when the application is built.
    Flag,
    /// Numeric restricted expression (int or float), read only.
    NumberValue,
    /// Boolean restricted expression.
    Bool,
    /// Writable session text field.
    TextField,
    /// Writable session bool field.
    BoolField,
    /// Writable session float field.
    NumberField,
    /// Writable session enum field whose variants all carry labels.
    PickerField,
    /// Writable session enum field selecting among child items.
    SelectionField,
    /// Value matching the enclosing selection field's enum.
    SelectionValue,
    /// List binding of records feeding a row template.
    Items,
    /// Field name of the row record used as stable key.
    KeyField,
    /// Writable session list of a route enum.
    PathField,
    /// Route value for the enclosing navigation path.
    Route,
    /// Window kind identifier.
    WindowId,
    /// Name of the generated window handle type.
    TypeName,
    /// Name of a slot that modules contribute to.
    SlotName,
    /// Name of a style variant the application declares in `style!`.
    StyleVariant,
    /// Name of a transition from `style!`, such as `pop` or `fade`.
    Transition,
    /// Name of a motion token, such as `smooth`.
    MotionToken,
    /// Name pairing two elements as one shared element.
    HeroName,
    /// A text, number or enum value identifying something, read only.
    KeyValue,
    /// A semantic color role, such as `accent` or `fill.subtle`.
    ColorRole,
    /// A list of numbers, read only, such as the values a `Series` draws.
    NumberList,
    /// Name of a pattern from `style!`.
    PatternName,
    /// Binding of a `MediaSource`, optional or not, followed by the player
    /// the declaration creates.
    MediaSource,
    /// An application player from the `#[bern::players]` catalog.
    Player,
    /// A typed query call such as `app.search(draft.text)`, whose result
    /// feeds the element's template.
    Query,
    /// A feature of the enclosing contract, such as `MediaFeature::Clip`.
    Feature,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropSpec {
    pub name: &'static str,
    pub kind: PropKind,
    pub required: bool,
}

/// What kind of children a component accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    None,
    /// Any nodes.
    Any,
    /// Exactly one node.
    One,
    /// Only the listed components (conditional nodes are not allowed).
    Only(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionRule {
    None,
    Required,
    Optional,
}

/// Where a component may appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Anywhere,
    /// Only as the root of a component body.
    Root,
    /// Only directly inside one of the listed parents.
    Within(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentSpec {
    pub name: &'static str,
    pub props: &'static [PropSpec],
    pub children: Children,
    pub template: bool,
    pub action: ActionRule,
    pub placement: Placement,
    /// Whether the component is an operable control that needs an `id`.
    pub interactive: bool,
    pub summary: &'static str,
}

impl ComponentSpec {
    pub fn prop(&self, name: &str) -> Option<&'static PropSpec> {
        self.props
            .iter()
            .find(|prop| prop.name == name)
            .or_else(|| {
                self.moves()
                    .then(|| {
                        MOTION_PROPS
                            .iter()
                            .chain(PLACEMENT_PROPS)
                            .find(|prop| prop.name == name)
                    })
                    .flatten()
            })
    }

    /// Whether the element takes the motion properties: everything shown
    /// in a layout, not window, slot or navigation structure.
    pub fn moves(&self) -> bool {
        !matches!(
            self.name,
            "WindowSet"
                | "Window"
                | "Layout"
                | "Slot"
                | "Pages"
                | "PageContent"
                | "Destination"
                | "Detail"
                | "Tab"
                | "MenuItem"
                | "SidebarItem"
                | "Section"
                | "MenuBar"
                | "CommandMenu"
                | "CommandItem"
        )
    }
}

pub use bern_protocol::shortcut::{SHORTCUT_KEYS, parse_shortcut};

/// File categories a control may accept besides `.ext` extensions.
pub const FILE_KINDS: &[&str] = &["any", "video", "audio", "picture", "folder"];

/// Extensions each file category takes besides the system's own types,
/// since a system may not know some, such as Matroska. Every target uses
/// this table.
pub const FILE_KIND_EXTENSIONS: &[(&str, &[&str])] = &[
    (
        "video",
        &[
            "mp4", "m4v", "mov", "mkv", "webm", "flv", "f4v", "avi", "wmv", "rmvb", "rm", "ts",
            "m2ts", "mts", "ogv", "3gp", "vob", "mpg", "mpeg",
        ],
    ),
    (
        "audio",
        &[
            "mp3", "m4a", "aac", "flac", "ape", "opus", "ogg", "oga", "wav", "aiff", "wma", "dsf",
            "dff",
        ],
    ),
    (
        "picture",
        &[
            "png", "jpg", "jpeg", "heic", "heif", "webp", "avif", "gif", "bmp", "tif", "tiff",
            "jxl",
        ],
    ),
];

/// The extensions of a file category from [`FILE_KIND_EXTENSIONS`].
pub fn file_kind_extensions(kind: &str) -> &'static [&'static str] {
    FILE_KIND_EXTENSIONS
        .iter()
        .find(|(name, _)| *name == kind)
        .map_or(&[], |(_, extensions)| extensions)
}

/// Normalize the kinds of file a control accepts: categories from
/// [`FILE_KINDS`] and extensions such as `.ass`, separated by spaces or
/// commas, each once.
pub fn parse_file_kinds(text: &str) -> Result<String, String> {
    let mut kinds: Vec<String> = Vec::new();
    for word in text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|word| !word.is_empty())
    {
        let word = word.to_lowercase();
        let known = match word.strip_prefix('.') {
            Some(ext) => {
                !ext.is_empty()
                    && ext.len() <= 16
                    && ext
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            }
            None => FILE_KINDS.contains(&word.as_str()),
        };
        if !known {
            return Err(format!(
                "`{word}` is neither a kind ({}) nor an extension such as `.srt`",
                FILE_KINDS.join(", ")
            ));
        }
        if kinds.contains(&word) {
            return Err(format!("`{word}` is written twice"));
        }
        kinds.push(word);
    }
    if kinds.is_empty() {
        return Err("name at least one kind, such as \"video\"".into());
    }
    if kinds.len() > 1 && kinds.iter().any(|kind| kind == "any") {
        return Err("`any` already takes every file".into());
    }
    Ok(kinds.join(" "))
}

/// Properties every element shown in a layout takes: how it appears and
/// disappears, how its children make room, and which shared element it is.
/// Navigation containers whose items may sit in `if`, `if let`, `match`
/// and `ForEach`; each branch and template holds only allowed items.
pub const DYNAMIC_ITEMS: &[&str] = &["Sidebar", "Tabs", "Toolbar", "CommandMenu"];

/// Roles of a `CommandMenu` in the menu bar: the system's menu of that
/// name takes the items, and the menu shows no title of its own.
pub const MENU_ROLES: &[&str] = &["app", "file", "edit", "view", "window", "help"];

/// Drawing components: a target without a canvas lists none of them.
pub const DRAWING: &[&str] = &["Canvas", "Shape", "Series", "Pattern"];

pub const MOTION_PROPS: &[PropSpec] = &[
    opt("appear", PropKind::Transition),
    opt("appear_delay", PropKind::Number),
    opt("disappear", PropKind::Transition),
    opt("reflow", PropKind::MotionToken),
    opt("hero", PropKind::HeroName),
    opt("hero_key", PropKind::KeyValue),
    opt("reveal", PropKind::Keyword(&["hover"])),
    opt("animate", PropKind::MotionToken),
    opt("appearance", PropKind::Keyword(&["light", "dark"])),
    opt("tint", PropKind::ColorRole),
    opt("scroll_scale", PropKind::Number),
    opt("scroll_opacity", PropKind::Number),
    opt("scroll_origin_x", PropKind::Number),
    opt("scroll_origin_y", PropKind::Number),
    opt("nudge", PropKind::Bool),
    opt("nudge_key", PropKind::KeyValue),
    opt("nudge_scale", PropKind::Number),
    opt("reduce_motion", PropKind::Bool),
    opt("stagger", PropKind::Number),
    // Seconds a mouse rests over the element, such as a player's picture,
    // before its window goes idle; with such areas on screen a mouse
    // anywhere else keeps the window awake.
    opt("idle_area", PropKind::Number),
];

/// Where an element sits on the `Rail` it stands in: at a fraction of its
/// length, or between two.
pub const PLACEMENT_PROPS: &[PropSpec] = &[
    opt("at", PropKind::NumberValue),
    opt("from", PropKind::NumberValue),
    opt("to", PropKind::NumberValue),
];

/// Containers that may stagger the appearing of their children.
pub const STAGGERING: &[&str] = &["Column", "Row", "Grid", "Stack", "ForEach", "List"];

const fn req(name: &'static str, kind: PropKind) -> PropSpec {
    PropSpec {
        name,
        kind,
        required: true,
    }
}

const fn opt(name: &'static str, kind: PropKind) -> PropSpec {
    PropSpec {
        name,
        kind,
        required: false,
    }
}

pub const TEXT_STYLES: &[&str] = &[
    "large_title",
    "title",
    "title2",
    "title3",
    "headline",
    "subheadline",
    "body",
    "callout",
    "footnote",
    "caption",
    "caption2",
    // The platform's control font, as a button's title has it: the text
    // of a capsule button's content matches the platform's own buttons.
    "control",
    // The platform's monospaced font, a little smaller than body: code,
    // commands, file contents.
    "code",
];

pub const EMPHASIS: &[&str] = &["primary", "secondary", "tertiary"];

/// Layout sizes of `Adaptive`, as written in `Layout(size: …)`: the four
/// sizes of the available width, and `tv`, the layout a television shows
/// whatever its width.
pub const LAYOUT_SIZES: &[&str] = &["compact", "medium", "expanded", "large", "tv"];

/// The feature an entry with a business action reaches.
const FEATURE: PropSpec = opt("feature", PropKind::Feature);

/// How video fills its frame: keep the aspect ratio and letterbox, keep it
/// and crop, or stretch.
pub const VIDEO_FIT: &[&str] = &["contain", "cover", "fill"];

/// Where the layers of a `Stack` sit when they are smaller than it.
pub const STACK_ALIGN: &[&str] = &[
    "top_leading",
    "top",
    "top_trailing",
    "leading",
    "center",
    "trailing",
    "bottom_leading",
    "bottom",
    "bottom_trailing",
];

/// File extensions an `Image` resource may have.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

const fn spec(
    name: &'static str,
    props: &'static [PropSpec],
    children: Children,
    summary: &'static str,
) -> ComponentSpec {
    ComponentSpec {
        name,
        props,
        children,
        template: false,
        action: ActionRule::None,
        placement: Placement::Anywhere,
        interactive: false,
        summary,
    }
}

pub const COMPONENTS: &[ComponentSpec] = &[
    ComponentSpec {
        placement: Placement::Root,
        ..spec(
            "WindowSet",
            &[
                req("handle", PropKind::TypeName),
                req("primary", PropKind::WindowId),
                opt("accent", PropKind::KeyValue),
                opt("palette", PropKind::KeyValue),
                opt("appearance", PropKind::KeyValue),
                opt("icon", PropKind::KeyValue),
            ],
            Children::Only(&["Window", "MenuBar"]),
            "Window catalog of a session root, with the application's `MenuBar`",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["WindowSet"]),
        ..spec(
            "MenuBar",
            &[],
            Children::Only(&["CommandMenu"]),
            "The application's menus in the system menu bar, where the platform has one (macOS; `window.menu_bar` holds there): each `CommandMenu` is a menu of its own, or with `role` adds its items to the system's menu of that name. Its items run their commands from any window; the drafts they read and write are the primary window's. Elsewhere it shows nothing",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["MenuBar", "CommandMenu"]),
        ..spec(
            "CommandMenu",
            &[
                opt("label", PropKind::LocalizedText),
                opt("role", PropKind::Keyword(MENU_ROLES)),
            ],
            Children::Only(&["CommandItem", "CommandMenu", "Divider"]),
            "A menu of the `MenuBar` titled `label`, or with `role` the system's menu of that name (`app`, `file`, `edit`, `view`, `window`, `help`) taking its items; inside another `CommandMenu` a submenu that opens beside its item; `Divider`s separate groups of items",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["CommandMenu"]),
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "CommandItem",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("shortcut", PropKind::Shortcut),
                opt("enabled", PropKind::Bool),
                opt("checked", PropKind::Bool),
                opt("pick", PropKind::FileField),
                opt("files", PropKind::FileKinds),
            ],
            Children::None,
            "A command of a `CommandMenu`: the menu shows its `shortcut`, greys it out while `enabled` is false and checks it while `checked` holds; `pick` asks for a file as `Button` does",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["WindowSet"]),
        action: ActionRule::Optional,
        ..spec(
            "Window",
            &[
                req("id", PropKind::WindowId),
                req("title", PropKind::LocalizedText),
                opt("initial", PropKind::Keyword(&["open", "closed"])),
                opt(
                    "presentation",
                    PropKind::Keyword(&["auto", "virtual", "floating", "native_required"]),
                ),
                opt(
                    "role",
                    PropKind::Keyword(&["standard", "settings", "dialog"]),
                ),
                opt("width", PropKind::Number),
                opt("height", PropKind::Number),
                opt("min_width", PropKind::Number),
                opt("min_height", PropKind::Number),
                opt("confirm_close", PropKind::Bool),
                opt("close_message", PropKind::LocalizedText),
                opt("chrome", PropKind::Keyword(&["standard", "unified"])),
                opt("immersive", PropKind::Bool),
                opt("landscape", PropKind::Bool),
                opt("idle_after", PropKind::Number),
                opt("idle_enabled", PropKind::Bool),
            ],
            Children::One,
            "Logical window with its own identity and state; `confirm_close` asks before closing, and the action saves",
        )
    },
    spec(
        "FocusRegion",
        &[],
        Children::One,
        "An editing region around one content view: while a text field inside is editing, the first pointer press outside ends editing without submitting or clearing and consumes the entire press; controls inside and subsequent outside presses work normally",
    ),
    spec(
        "Column",
        &[
            opt("gap", PropKind::Length),
            opt("padding", PropKind::Length),
            opt("padding_x", PropKind::Length),
            opt("padding_y", PropKind::Length),
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt(
                "align",
                PropKind::Keyword(&["leading", "center", "trailing"]),
            ),
            opt("variant", PropKind::StyleVariant),
        ],
        Children::Any,
        "Vertical arrangement; a `style!` variant may set its lengths per platform",
    ),
    spec(
        "Row",
        &[
            opt("gap", PropKind::Length),
            opt("padding", PropKind::Length),
            opt("padding_x", PropKind::Length),
            opt("padding_y", PropKind::Length),
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt(
                "align",
                PropKind::Keyword(&["top", "center", "bottom", "baseline"]),
            ),
            opt("variant", PropKind::StyleVariant),
        ],
        Children::Any,
        "Horizontal arrangement; `align: baseline` lines the texts up on the baseline of their first line; a `style!` variant may set its lengths per platform",
    ),
    spec(
        "Accordion",
        &[
            req("count", PropKind::NumberValue),
            req("selected", PropKind::NumberValue),
            req("collapsed_width", PropKind::Length),
            opt("height", PropKind::Length),
            opt("gap", PropKind::Length),
            opt("corner", PropKind::Length),
        ],
        Children::Any,
        "Horizontal accordion: one column expands into the remaining width; without height it fills the available height; animated widths share a layout and fixed gaps",
    ),
    spec(
        "Grid",
        &[
            req("min_width", PropKind::Length),
            opt("gap", PropKind::Length),
            opt("row_gap", PropKind::Length),
            opt("padding", PropKind::Length),
            opt("padding_x", PropKind::Length),
            opt("padding_y", PropKind::Length),
            opt("cell_ratio", PropKind::Number),
            opt("cell_extra", PropKind::Length),
            opt("masonry", PropKind::Flag),
        ],
        Children::Any,
        "Equal columns, as many as fit with each at least `min_width` wide; children fill them row by row; with `cell_ratio` every row is the column width ÷ `cell_ratio` + `cell_extra` tall, its children at the top, and taller content runs into the gap below; with `masonry` each child is as tall as it is and stands under the column that ends highest so far, the leftmost of those that tie",
    ),
    spec(
        "Stack",
        &[
            opt("variant", PropKind::StyleVariant),
            opt("enabled", PropKind::Bool),
            opt("align", PropKind::Keyword(STACK_ALIGN)),
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt("under_bars", PropKind::Flag),
        ],
        Children::Any,
        "Layers on top of each other, later children above earlier ones; as large as the largest unless `width` or `height` says. With `under_bars: true` it fills the window to its edges while it is shown: under the title bar and toolbar of a desktop window, under a phone's status bar and home indicator, as a scene drawn in window coordinates does",
    ),
    spec(
        "Marquee",
        &[
            req("duration", PropKind::Length),
            opt("phase", PropKind::Number),
            opt("paused", PropKind::Bool),
            opt("opacity", PropKind::NumberValue),
        ],
        Children::Any,
        "A clipped horizontal scrolling line; the renderer owns its clock and pause state",
    ),
    spec(
        "ScaleBox",
        &[
            req("width", PropKind::Length),
            req("height", PropKind::Length),
        ],
        Children::Any,
        "A fixed logical coordinate space, uniformly fitted and centered in the available size",
    ),
    spec(
        "Positioned",
        &[
            req("x", PropKind::Length),
            req("y", PropKind::Length),
            req("width", PropKind::Length),
            req("height", PropKind::Length),
            opt("bleed", PropKind::Bool),
        ],
        Children::Any,
        "A layer positioned from the top leading corner of its containing coordinate space; with `bleed`, a layer of a `ScaleBox` scene continues past each scene edge it reaches, to the edge of the box",
    ),
    spec(
        "Modal",
        &[
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt("cover", PropKind::Keyword(&["compact"])),
        ],
        Children::Any,
        "Centered modal content, constrained to the hosting window, above a dimmed input-blocking backdrop; without `height` it is as tall as its content, a scroll view in it as tall as what it scrolls, until the window is too short; with `cover: compact` it covers the whole window where the window is compact, as a phone's full-screen flow",
    ),
    spec(
        "Flow",
        &[opt("gap", PropKind::Length)],
        Children::Any,
        "Leading-aligned items wrapping onto new rows at the available width; each row centres its items, and a `Spacer` takes the room left at the end of its row",
    ),
    ComponentSpec {
        action: ActionRule::Required,
        ..spec(
            "Timer",
            &[
                req("interval", PropKind::Number),
                opt("enabled", PropKind::Bool),
            ],
            Children::None,
            "Invoke a command at an interval in seconds while mounted and enabled; automatically cancels on removal",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        ..spec(
            "Shortcut",
            &[
                req("key", PropKind::Shortcut),
                opt("enabled", PropKind::Bool),
            ],
            Children::None,
            "A keyboard shortcut without a control of its own: the key runs the action while the shortcut is mounted and enabled; on a television, the remote's button that stands for the key",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Picker"]),
        ..spec(
            "PickerOption",
            &[
                req("value", PropKind::KeyValue),
                req("label", PropKind::DisplayText),
            ],
            Children::None,
            "An explicitly labelled picker option, including live counts",
        )
    },
    spec(
        "Rail",
        &[
            opt("axis", PropKind::Keyword(&["horizontal", "vertical"])),
            opt("anchor", PropKind::Keyword(&["start", "center", "end"])),
            opt("overhang", PropKind::Length),
        ],
        Children::Any,
        "A strip along `axis` that places each child at the fraction `at` of its length, or between the fractions `from` and `to`; the fractions may follow state; with `overhang` the scale runs that far past both ends, fraction 0 standing before the rail's start",
    ),
    spec("Spacer", &[], Children::None, "Flexible space"),
    spec(
        "WindowButtons",
        &[],
        Children::None,
        "The room the window's own buttons take at the leading end of content that runs under the title bar: the traffic lights on macOS; nothing, and no gap, where the window has no buttons there",
    ),
    spec("Divider", &[], Children::None, "Separator line"),
    spec(
        "ScrollView",
        &[
            opt("axis", PropKind::Keyword(&["vertical", "horizontal"])),
            opt("snap", PropKind::Keyword(&["start", "center"])),
            opt("position", PropKind::PositionField),
            opt("arrival", PropKind::Keyword(&["nudge"])),
            opt("indicators", PropKind::Keyword(&["automatic", "hidden"])),
            opt("open_at", PropKind::Number),
            opt("open_center", PropKind::NumberValue),
        ],
        Children::Any,
        "Scrolling container; `open_at` is where it opens: the point at that fraction of the content at the same fraction of the view, 0 the start, 0.5 the middle at the middle, 1 the end; `open_center` opens with the point at that fraction of the content in the middle of the view, or as near the middle as the content reaches, so the point is always in view; it may be bound, and is read once as the view opens; with `snap`, scrolling comes to rest with an item at the start or center; `position` holds the key of the item there, and setting it scrolls to that item, which with `arrival: nudge` gives a nudge when it arrives; `indicators: hidden` scrolls without showing the scroll bar, for a shelf that has its own paging buttons",
    ),
    spec(
        "SplitView",
        &[],
        Children::Only(&["Sidebar", "Detail"]),
        "Sidebar and detail columns",
    ),
    ComponentSpec {
        placement: Placement::Within(&["SplitView"]),
        ..spec(
            "Sidebar",
            &[
                req("selection", PropKind::SelectionField),
                opt("width", PropKind::Length),
                opt("id", PropKind::Id),
                opt("fallback", PropKind::SelectionValue),
            ],
            Children::Only(&[
                "Search",
                "Section",
                "SidebarItem",
                "Pages",
                "SidebarHeader",
                "SidebarFooter",
            ]),
            "Sidebar list selecting a destination",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Sidebar"]),
        ..spec(
            "SidebarHeader",
            &[opt("role", PropKind::Keyword(&["content", "title"]))],
            Children::Any,
            "Content pinned to the top of the sidebar, above its list, such as the application's name and icon; `role: title` says it only names the window with the application's icon and name, so platforms whose title bar already shows both (Windows) leave it out",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Sidebar"]),
        ..spec(
            "SidebarFooter",
            &[],
            Children::Any,
            "Content pinned to the bottom of the sidebar, below its list, top to bottom",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["SplitView"]),
        ..spec(
            "Detail",
            &[opt("titled", PropKind::Bool)],
            Children::Any,
            "Detail column; `titled: false` keeps the selected destination's name out of its title bar, for pages that show their own title",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Sidebar", "Section"]),
        ..spec(
            "SidebarItem",
            &[
                req("value", PropKind::SelectionValue),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("badge", PropKind::NumberValue),
                opt("variant", PropKind::StyleVariant),
            ],
            Children::None,
            "Selectable sidebar destination",
        )
    },
    spec(
        "Section",
        &[
            opt("title", PropKind::DisplayText),
            opt("footer", PropKind::DisplayText),
            opt("style", PropKind::Keyword(&["grouped", "plain"])),
            opt("padding_x", PropKind::Length),
            opt("padding_y", PropKind::Length),
        ],
        Children::Any,
        "Titled group inside a form, list or sidebar; `style: plain` sets its rows on the page itself, without the group's background, margins or separators, as a heading that scrolls with the form, and the form's row insets do not reach them; `padding_x` and `padding_y` are its rows' insets",
    ),
    spec(
        "Slot",
        &[req("id", PropKind::SlotName)],
        Children::None,
        "Where the application's modules contribute components, in the order Bern.toml lists the modules",
    ),
    ComponentSpec {
        placement: Placement::Within(&["Sidebar", "Section", "Tabs"]),
        ..spec(
            "Pages",
            &[req("slot", PropKind::SlotName)],
            Children::None,
            "One sidebar item, or in `Tabs` one tab showing the page, for every page the modules contribute to a page slot",
        )
    },
    spec(
        "PageContent",
        &[
            req("slot", PropKind::SlotName),
            req("selected", PropKind::SelectionField),
        ],
        Children::None,
        "The contributed page of a page slot that `selected` names; nothing for the application's own pages",
    ),
    spec(
        "Form",
        &[
            opt("title", PropKind::DisplayText),
            opt("variant", PropKind::StyleVariant),
            opt("gap", PropKind::Length),
            opt("row_height", PropKind::Length),
            opt("padding_x", PropKind::Length),
            opt("padding_y", PropKind::Length),
        ],
        Children::Any,
        "Grouped form of labelled controls, optionally titling its page; `gap` is the room between its sections, `row_height` the least height of a row, and `padding_x` / `padding_y` the rows' insets in every section; a form opening with a plain section starts at the top of its page, also when a slot, a component or a condition gives that section",
    ),
    spec(
        "Navigation",
        &[req("path", PropKind::PathField), opt("id", PropKind::Id)],
        Children::Any,
        "Push navigation bound to a typed route path",
    ),
    ComponentSpec {
        template: true,
        placement: Placement::Within(&["Navigation"]),
        ..spec(
            "Destination",
            &[],
            Children::Any,
            "Content shown for a pushed route",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "NavLink",
            &[req("id", PropKind::Id), req("route", PropKind::Route)],
            Children::Any,
            "Row that pushes a route",
        )
    },
    spec(
        "ControlGroup",
        &[
            opt("label", PropKind::DisplayText),
            opt("height", PropKind::Length),
        ],
        Children::Any,
        "Related native controls sharing one capsule surface and equal-height hit targets",
    ),
    spec(
        "Tabs",
        &[
            req("selection", PropKind::SelectionField),
            opt("id", PropKind::Id),
            opt("fallback", PropKind::SelectionValue),
        ],
        Children::Only(&["Tab", "Pages", "TabAccessory"]),
        "Sibling pages switched by tabs",
    ),
    ComponentSpec {
        placement: Placement::Within(&["Tabs"]),
        ..spec(
            "TabAccessory",
            &[opt("shown", PropKind::Bool)],
            Children::Any,
            "A bar kept with the tabs over every page, such as what is playing",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Tabs"]),
        ..spec(
            "Tab",
            &[
                req("value", PropKind::SelectionValue),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("display", PropKind::Keyword(&["title_and_icon", "icon"])),
            ],
            Children::Any,
            "One tab page; `display: icon` shows only its icon in the tab bar, the label staying its accessible name",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "SplitButton",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("value", PropKind::BoolField),
                opt("checked", PropKind::Bool),
                opt("icon", PropKind::Icon),
                opt("shape", PropKind::Keyword(&["capsule"])),
                opt("height", PropKind::Length),
                opt("more_label", PropKind::DisplayText),
                opt("shortcut", PropKind::Shortcut),
            ],
            Children::Any,
            "A toggle with a second part beside it that shows its children in a popover, such as the settings of what it turns on; like a `Switch`, it toggles a session field (`value`) or shows engine state (`checked`) whose change runs the action",
        )
    },
    ComponentSpec {
        interactive: true,
        action: ActionRule::Optional,
        ..spec(
            "PopoverButton",
            &[
                opt(
                    "panel_appearance",
                    PropKind::Keyword(&["inherit", "window", "surface"]),
                ),
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("presented", PropKind::BoolField),
                opt("open", PropKind::Bool),
                opt("shortcut", PropKind::Shortcut),
            ],
            Children::Any,
            "A button that shows its children in a popover; `presented` binds a session field, or `open` follows engine state and commits opening and dismissal through its action",
        )
    },
    spec(
        "Inspector",
        &[
            req("presented", PropKind::BoolField),
            opt("width", PropKind::Number),
        ],
        Children::Any,
        "A side panel at the trailing edge of the container it stands in, shown while `presented` is true; the container's other content makes room",
    ),
    spec(
        "Toast",
        &[
            req("content", PropKind::DisplayText),
            opt("icon", PropKind::Icon),
            opt("key", PropKind::KeyValue),
            opt("duration", PropKind::Number),
        ],
        Children::None,
        "A short notice that shows when it appears or its `key` (else its content) changes, and hides itself after `duration` seconds (3 unless given)",
    ),
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Pinch",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::NumberField),
                req("min", PropKind::Number),
                req("max", PropKind::Number),
                FEATURE,
            ],
            Children::One,
            "Pinching its content scales `value` within `min` and `max`; the action runs when the fingers lift",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Swipe",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::NumberField),
                req("min", PropKind::Number),
                req("max", PropKind::Number),
                opt("axis", PropKind::Keyword(&["vertical", "horizontal"])),
                opt("distance", PropKind::Number),
                FEATURE,
            ],
            Children::One,
            "Dragging across its content moves `value` within `min` and `max`: up or right raises it, and `distance` points (300 unless given) cross the whole range; the action runs when the finger lifts",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "Drop",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                req("value", PropKind::FileField),
                opt("files", PropKind::FileKinds),
                opt("over", PropKind::BoolField),
                opt("opens", PropKind::Flag),
                FEATURE,
            ],
            Children::One,
            "Its content accepts a file of `files` dragged onto it: `over` is true while one is held above it, and on dropping the file goes into `value` and the action runs. With `opens: true` it also takes the files the system opens with the application, which then registers for those kinds",
        )
    },
    spec(
        "Toolbar",
        &[],
        Children::Only(&[
            "Button",
            "IconButton",
            "Picker",
            "Menu",
            "Search",
            "Switch",
            "SplitButton",
            "PopoverButton",
            "Divider",
            "Spacer",
            "Text",
        ]),
        "Window or page level actions, and at most one search field; items between `Divider`s form one group. Items before a `Spacer` stand at the leading end, where a page's way back is, and the rest at the trailing end",
    ),
    spec(
        "Adaptive",
        &[opt("id", PropKind::Id)],
        Children::Only(&["Layout"]),
        "Chooses one `Layout` per size by the available width, or the `tv` layout on a television; state is kept across sizes",
    ),
    ComponentSpec {
        placement: Placement::Within(&["Adaptive"]),
        ..spec(
            "Layout",
            &[req("size", PropKind::Keyword(LAYOUT_SIZES))],
            Children::Any,
            "Content of one layout size",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "Menu",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("hide_disabled", PropKind::Flag),
                opt("icon", PropKind::Icon),
                opt("style", PropKind::Keyword(&["button", "icon"])),
                opt("shape", PropKind::Keyword(&["capsule"])),
                opt("height", PropKind::Length),
            ],
            Children::Only(&["MenuItem", "Divider"]),
            "Button opening a menu of commands; `style: icon` shows only the icon, as an `IconButton`, with `label` as its accessible name, and with `height` is a round button of that diameter",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Menu"]),
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "MenuItem",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("role", PropKind::Keyword(&["normal", "destructive"])),
                opt("enabled", PropKind::Bool),
                FEATURE,
                opt("shortcut", PropKind::Shortcut),
                opt("pick", PropKind::FileField),
                opt("files", PropKind::FileKinds),
            ],
            Children::None,
            "Menu command bound to a registered command; `pick` asks for a file as `Button` does",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "LanguagePicker",
            &[req("id", PropKind::Id), req("label", PropKind::DisplayText)],
            Children::None,
            "Switches the interface language among the supported languages at run time",
        )
    },
    ComponentSpec {
        template: true,
        ..spec(
            "List",
            &[
                opt("items", PropKind::Items),
                opt("key", PropKind::KeyField),
            ],
            Children::Any,
            "List of rows, static or keyed by data",
        )
    },
    ComponentSpec {
        template: true,
        ..spec(
            "PreviewList",
            &[
                req("items", PropKind::Items),
                req("key", PropKind::KeyField),
                req("item_height", PropKind::Number),
                opt("gap", PropKind::Number),
                opt("more_height", PropKind::Number),
            ],
            Children::Any,
            "A bounded vertical preview: creates only complete rows fitting its height, reserving a +N badge for omitted items; does not scroll",
        )
    },
    ComponentSpec {
        template: true,
        ..spec(
            "ForEach",
            &[
                req("items", PropKind::Items),
                req("key", PropKind::KeyField),
            ],
            Children::Any,
            "Keyed repetition inside any container",
        )
    },
    spec(
        "Text",
        &[
            req("content", PropKind::DisplayText),
            opt("style", PropKind::Keyword(TEXT_STYLES)),
            opt("emphasis", PropKind::Keyword(EMPHASIS)),
            opt("mark", PropKind::ColorRole),
            opt("lines", PropKind::Number),
            opt("font_size", PropKind::Length),
            opt("line_height", PropKind::Length),
            opt("markdown", PropKind::Flag),
        ],
        Children::None,
        "Text with a semantic style; `mark` draws a band of the color under each line; `lines` shows at most that many lines, ending the last with an ellipsis; `line_height` sets the height of each line, the extra split above and below it as in CSS; with `markdown` the content's inline Markdown — `**strong**`, `*emphasis*`, `` `code` ``, `~~struck~~` and `[links](address)`, a backslash taking the next mark literally — shows as the platform styles it",
    ),
    spec(
        "Canvas",
        &[
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt("label", PropKind::DisplayText),
        ],
        Children::Only(&["Shape", "Series", "Pattern"]),
        "A drawing of shapes, data series and patterns in order; `label` describes what it shows, without one it is decorative",
    ),
    ComponentSpec {
        placement: Placement::Within(&["Canvas"]),
        ..spec(
            "Shape",
            &[
                req(
                    "kind",
                    PropKind::Keyword(&["rect", "rounded", "capsule", "ellipse"]),
                ),
                opt("x", PropKind::Number),
                opt("y", PropKind::Number),
                opt("width", PropKind::Number),
                opt("height", PropKind::Number),
                opt("corner", PropKind::Number),
                opt("fill", PropKind::ColorRole),
                opt("fill_from", PropKind::ImageResource),
                opt("fade", PropKind::Keyword(&["up", "down", "left", "right"])),
                opt("stroke", PropKind::ColorRole),
                opt("stroke_width", PropKind::Number),
                opt("dash", PropKind::Number),
                opt("dash_gap", PropKind::Number),
                opt("opacity", PropKind::Number),
            ],
            Children::None,
            "A shape in canvas units; without a size it fills the canvas. `fill_from` fills it with the average color of a picture; `fade` lets the fill fade out toward an edge; `dash` dashes the stroke, dashes that long with gaps of `dash_gap`, by default as long",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Canvas"]),
        ..spec(
            "Series",
            &[
                req("values", PropKind::NumberList),
                opt("kind", PropKind::Keyword(&["area", "line"])),
                opt("max", PropKind::NumberValue),
                opt("fill", PropKind::ColorRole),
                opt("stroke", PropKind::ColorRole),
                opt("stroke_width", PropKind::Number),
                opt("opacity", PropKind::Number),
            ],
            Children::None,
            "Numbers drawn across the canvas width as an area or a line, scaled to `max` (their largest without it)",
        )
    },
    ComponentSpec {
        placement: Placement::Within(&["Canvas"]),
        ..spec(
            "Pattern",
            &[
                req("name", PropKind::PatternName),
                opt("opacity", PropKind::Number),
            ],
            Children::None,
            "A pattern from `style!` tiled over the canvas",
        )
    },
    spec(
        "Label",
        &[
            req("title", PropKind::DisplayText),
            req("icon", PropKind::Icon),
            opt("style", PropKind::Keyword(TEXT_STYLES)),
            opt("font_size", PropKind::Number),
            opt("display", PropKind::Keyword(&["title_and_icon", "icon"])),
        ],
        Children::None,
        "Icon and title; `display: icon` hides the title visually while preserving its accessible name",
    ),
    spec(
        "Svg",
        &[
            req("source", PropKind::SvgResource),
            opt("static_source", PropKind::SvgResource),
            req("width", PropKind::Length),
            req("height", PropKind::Length),
            opt("label", PropKind::DisplayText),
            opt("fit", PropKind::Keyword(VIDEO_FIT)),
            opt("playing", PropKind::Bool),
            opt("speed", PropKind::NumberValue),
            opt("restart", PropKind::NumberValue),
            opt("interactive", PropKind::Flag),
            opt("data", PropKind::DisplayText),
            opt("external_links", PropKind::Flag),
        ],
        Children::None,
        "Bundled SVG document with CSS, SMIL and script animation; explicit viewport, playing/speed controls and restart revision; document-local interaction is enabled by default",
    ),
    spec(
        "Image",
        &[
            req("source", PropKind::ImageResource),
            opt("label", PropKind::DisplayText),
            opt("width", PropKind::Length),
            opt("height", PropKind::Length),
            opt("fit", PropKind::Keyword(VIDEO_FIT)),
            opt("corner", PropKind::Length),
            opt("rendering", PropKind::Keyword(&["original", "template"])),
            opt("aspect", PropKind::Number),
        ],
        Children::None,
        "Picture from a declared resource, sized by `width` and/or `height`, its corners rounded by `corner`; `rendering: template` uses its alpha as a mask for the inherited text color, otherwise it keeps its colors; `label` describes it, without one it is decorative",
    ),
    spec(
        "Progress",
        &[
            opt("value", PropKind::NumberValue),
            opt("total", PropKind::Number),
            opt("label", PropKind::DisplayText),
        ],
        Children::None,
        "Progress of `value` out of `total` (1 unless given) as a bar; without `value`, an indeterminate spinner",
    ),
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "Button",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("shape", PropKind::Keyword(&["capsule", "chip"])),
                opt("height", PropKind::Length),
                opt("width", PropKind::Keyword(&["fill"])),
                opt(
                    "role",
                    PropKind::Keyword(&["normal", "destructive", "cancel"]),
                ),
                opt(
                    "prominence",
                    PropKind::Keyword(&["standard", "prominent", "plain"]),
                ),
                opt("enabled", PropKind::Bool),
                opt("variant", PropKind::StyleVariant),
                FEATURE,
                opt("shortcut", PropKind::Shortcut),
                opt("pick", PropKind::FileField),
                opt("files", PropKind::FileKinds),
                opt("default_focus", PropKind::Flag),
                opt("clears", PropKind::TextField),
            ],
            Children::Any,
            "Button bound to a registered command; with one child, the child is what it shows and `label` its accessible name; `width: fill` stretches the system button across its container; with `pick`, it first asks for a file of `files` and runs the action once one is chosen; `default_focus: true` starts the focus on it when it appears (a television's remote, or a keyboard); `clears: draft.text` empties that draft after the action and gives the keyboard back to the window, as a composer's send button does",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "IconButton",
            &[
                req("id", PropKind::Id),
                req("icon", PropKind::Icon),
                opt("font_size", PropKind::Number),
                opt("height", PropKind::Length),
                req("label", PropKind::DisplayText),
                opt(
                    "prominence",
                    PropKind::Keyword(&["standard", "prominent", "plain"]),
                ),
                opt("enabled", PropKind::Bool),
                opt("variant", PropKind::StyleVariant),
                FEATURE,
                opt("shortcut", PropKind::Shortcut),
                opt("pick", PropKind::FileField),
                opt("files", PropKind::FileKinds),
                opt("default_focus", PropKind::Flag),
            ],
            Children::None,
            "Icon-only button with an accessible name; `prominence: plain` draws no background, as the icons at the foot of a sidebar; `height` makes it a round system button of that diameter; `pick` asks for a file as `Button` does; `default_focus: true` starts the focus on it as `Button` does",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "TextInput",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("label_hidden", PropKind::Flag),
                req("value", PropKind::TextField),
                opt("prompt", PropKind::DisplayText),
                opt("kind", PropKind::Keyword(&["plain", "secure", "multiline"])),
                opt("submit", PropKind::Keyword(&["clear"])),
                opt("clear_key", PropKind::NumberValue),
                FEATURE,
            ],
            Children::None,
            "Text editing bound to a session draft; the action runs on submit, and with `submit: clear` the draft then empties and the field gives up the keyboard, as a message composer does",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Search",
            &[
                req("id", PropKind::Id),
                req("value", PropKind::TextField),
                opt("prompt", PropKind::DisplayText),
                FEATURE,
                opt("shortcut", PropKind::Shortcut),
                opt("style", PropKind::Keyword(&["automatic", "palette"])),
                opt("panel_prompt", PropKind::DisplayText),
                opt("width", PropKind::Number),
                opt("height", PropKind::Number),
                opt("font_size", PropKind::Number),
                opt("suggest", PropKind::Flag),
            ],
            Children::Any,
            "Search text bound to a session draft; the action runs on submit; children are the results shown below the field while it is in use; with `suggest: true` they show before anything is typed too (on a television, whose search page types with the remote, always); a palette's `width` and `height` size its panel, a field in the page's its field, and `font_size` its text",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "SearchItem",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
                opt("detail", PropKind::DisplayText),
                opt("hint", PropKind::DisplayText),
                opt("navigation", PropKind::Bool),
                opt("pick", PropKind::FileField),
                opt("files", PropKind::FileKinds),
                FEATURE,
            ],
            Children::Any,
            "A palette result; activates and dismisses the search; optional content replaces the row; navigation false excludes it from arrow navigation",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Switch",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("value", PropKind::BoolField),
                opt("checked", PropKind::Bool),
                opt("label_hidden", PropKind::Flag),
                opt("shape", PropKind::Keyword(&["capsule", "chip"])),
                opt("height", PropKind::Length),
                opt(
                    "style",
                    PropKind::Keyword(&["switch", "checkbox", "button"]),
                ),
                opt("icon", PropKind::Icon),
                opt("shortcut", PropKind::Shortcut),
            ],
            Children::None,
            "Boolean state; `style: button` is a button that stays pressed while on",
        )
    },
    ComponentSpec {
        action: ActionRule::Required,
        interactive: true,
        ..spec(
            "ShortcutInput",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("label_hidden", PropKind::Flag),
                req("current", PropKind::Shortcut),
                opt("prompt", PropKind::DisplayText),
                opt("empty", PropKind::DisplayText),
                opt("enabled", PropKind::Bool),
                FEATURE,
            ],
            Children::None,
            "A keyboard shortcut chosen by pressing it: a button showing `current` as the platform writes shortcuts (`empty` while there is none); pressed, it shows `prompt` and takes the next key with its modifiers, Escape leaving it as it was and Delete alone clearing it; the action receives the new shortcut in place of `current`",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Slider",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("label_hidden", PropKind::Flag),
                opt("value", PropKind::NumberField),
                opt("current", PropKind::NumberValue),
                opt("continuous", PropKind::Flag),
                req("min", PropKind::Number),
                req("max", PropKind::Number),
                opt("step", PropKind::Number),
                opt("min_icon", PropKind::Icon),
                opt("max_icon", PropKind::Icon),
                FEATURE,
            ],
            Children::None,
            "Range value previewed in the session; the action runs on commit",
        )
    },
    ComponentSpec {
        interactive: true,
        ..spec(
            "Video",
            &[
                req("id", PropKind::Id),
                opt("source", PropKind::MediaSource),
                opt("player", PropKind::Player),
                opt("label", PropKind::DisplayText),
                opt("controls", PropKind::Keyword(&["standard", "none"])),
                opt("fit", PropKind::Keyword(VIDEO_FIT)),
                opt("sizing", PropKind::Keyword(&["standard", "fill"])),
                opt("corner", PropKind::Length),
            ],
            Children::None,
            "Video of a player created from `source` or attached with `player`; `sizing: fill` fills a bounded container independently of the picture's `fit`; `corner` rounds the picture",
        )
    },
    ComponentSpec {
        template: true,
        interactive: true,
        ..spec(
            "Query",
            &[
                req("id", PropKind::Id),
                req("query", PropKind::Query),
                opt("loading", PropKind::DisplayText),
                opt("empty", PropKind::DisplayText),
                opt("failed", PropKind::DisplayText),
                opt("icon", PropKind::Icon),
            ],
            Children::Any,
            "Result of an asynchronous query: loading, ready, refreshing, failed and empty states; the template shows the data",
        )
    },
    ComponentSpec {
        action: ActionRule::Optional,
        interactive: true,
        ..spec(
            "Picker",
            &[
                req("id", PropKind::Id),
                req("label", PropKind::DisplayText),
                opt("value", PropKind::PickerField),
                opt("selected", PropKind::KeyValue),
                opt("label_hidden", PropKind::Flag),
                opt("shape", PropKind::Keyword(&["capsule"])),
                opt("height", PropKind::Length),
                opt(
                    "style",
                    PropKind::Keyword(&[
                        "automatic",
                        "menu",
                        "segmented",
                        "radio",
                        "inline",
                        "chips",
                    ]),
                ),
            ],
            Children::Only(&["PickerOption"]),
            "Single choice among the labelled variants of an enum, with optional explicit labels; `style: chips` shows a chip per option, which in a `Flow` are items of the flow",
        )
    },
];

pub fn component(name: &str) -> Option<&'static ComponentSpec> {
    COMPONENTS.iter().find(|spec| spec.name == name)
}

/// One structural problem found by [`check_shape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeError {
    /// Property the problem refers to, if any.
    pub prop: Option<String>,
    pub message: String,
}

/// The syntactic outline of an element, as seen by any parser.
pub struct ElementShape<'a> {
    pub name: &'a str,
    pub props: Vec<&'a str>,
    pub has_template: bool,
    pub children: Option<Vec<ChildShape<'a>>>,
    pub has_action: bool,
    pub parent: Option<&'a str>,
    pub is_root: bool,
}

pub enum ChildShape<'a> {
    Element(&'a str),
    Other,
}

/// Validate an element against its specification. Shared by the macros
/// (for early, span-accurate errors) and the compiler.
pub fn check_shape(shape: &ElementShape<'_>) -> Vec<ShapeError> {
    let mut errors = Vec::new();
    let Some(spec) = component(shape.name) else {
        let known: Vec<_> = COMPONENTS.iter().map(|spec| spec.name).collect();
        errors.push(ShapeError {
            prop: None,
            message: format!(
                "unknown component `{}`; built-in components are: {}",
                shape.name,
                known.join(", ")
            ),
        });
        return errors;
    };
    for prop in &shape.props {
        if spec.prop(prop).is_none() {
            let known: Vec<_> = spec.props.iter().map(|prop| prop.name).collect();
            errors.push(ShapeError {
                prop: Some(prop.to_string()),
                message: if known.is_empty() {
                    format!("`{}` takes no properties", spec.name)
                } else {
                    format!(
                        "`{}` has no property `{prop}`; expected one of: {}",
                        spec.name,
                        known.join(", ")
                    )
                },
            });
        }
    }
    for prop in spec.props.iter().filter(|prop| prop.required) {
        if !shape.props.contains(&prop.name) {
            errors.push(ShapeError {
                prop: None,
                message: format!("`{}` requires the property `{}`", spec.name, prop.name),
            });
        }
    }
    let list_needs_template = spec.name == "List" && shape.props.contains(&"items");
    if shape.has_template && !spec.template {
        errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take a row template `|item|`", spec.name),
        });
    }
    if spec.template && !shape.has_template && (spec.name != "List" || list_needs_template) {
        errors.push(ShapeError {
            prop: None,
            message: format!("`{}` needs a template variable such as `|item|`", spec.name),
        });
    }
    if spec.name == "List" && shape.has_template && !list_needs_template {
        errors.push(ShapeError {
            prop: None,
            message: "a static `List` has no row template; add `items:` and `key:`".into(),
        });
    }
    if spec.name == "List" && shape.props.contains(&"items") != shape.props.contains(&"key") {
        errors.push(ShapeError {
            prop: None,
            message: "`List` needs both `items:` and `key:` for data rows".into(),
        });
    }
    if spec.name == "Image"
        && !shape.props.contains(&"width")
        && !shape.props.contains(&"height")
        && !shape.props.contains(&"aspect")
    {
        errors.push(ShapeError {
            prop: None,
            message: "`Image` needs `width:`, `height:` or `aspect:`; with one, the other follows the picture's aspect ratio".into(),
        });
    }
    if spec.name == "Button" && shape.children.is_some() {
        if shape.children.as_deref().unwrap_or(&[]).len() != 1 {
            errors.push(ShapeError {
                prop: None,
                message: "a `Button` shows one child; put several in a `Column`, `Row` or `Stack`"
                    .into(),
            });
        }
        for prop in ["icon", "prominence"] {
            if shape.props.contains(&prop) {
                errors.push(ShapeError {
                    prop: Some(prop.into()),
                    message: format!(
                        "a `Button` with content draws no label of its own, so `{prop}:` has no effect; show it in the content"
                    ),
                });
            }
        }
    }
    if shape.props.contains(&"files")
        && !shape.props.contains(&"pick")
        && [
            "Button",
            "IconButton",
            "MenuItem",
            "SearchItem",
            "CommandItem",
        ]
        .contains(&spec.name)
    {
        errors.push(ShapeError {
            prop: Some("files".into()),
            message: format!(
                "`files:` says which files `pick:` asks for; add `pick: draft.file` to the `{}`",
                spec.name
            ),
        });
    }
    if spec.name == "CommandMenu" {
        let role = shape.props.contains(&"role");
        if role == shape.props.contains(&"label") {
            errors.push(ShapeError {
                prop: None,
                message: "a `CommandMenu` takes `label:` for a menu of its own, or `role:` to add its items to the system's menu of that name".into(),
            });
        }
        if role && shape.parent != Some("MenuBar") {
            errors.push(ShapeError {
                prop: Some("role".into()),
                message: "`role:` names a menu of the menu bar; a submenu takes `label:`".into(),
            });
        }
    }
    if spec.name == "ScrollView"
        && shape.props.contains(&"arrival")
        && !shape.props.contains(&"position")
    {
        errors.push(ShapeError {
            prop: Some("arrival".into()),
            message: "`arrival` nudges the item that setting `position` scrolls to; add `position: draft.x`".into(),
        });
    }
    if spec.name == "Progress" && shape.props.contains(&"total") && !shape.props.contains(&"value")
    {
        errors.push(ShapeError {
            prop: Some("total".into()),
            message:
                "`total:` needs a `value:`; without one, `Progress` is an indeterminate spinner"
                    .into(),
        });
    }
    if shape.props.contains(&"nudge_scale") && !shape.props.contains(&"nudge_key") {
        errors.push(ShapeError {
            prop: Some("nudge_scale".into()),
            message: "`nudge_scale` needs a `nudge_key` event binding".into(),
        });
    }
    if shape.props.contains(&"nudge") && shape.props.contains(&"nudge_key") {
        errors.push(ShapeError {
            prop: Some("nudge_key".into()),
            message: "choose `nudge` for a condition or `nudge_key` for repeated events".into(),
        });
    }
    if shape.props.contains(&"appear_delay") && !shape.props.contains(&"appear") {
        errors.push(ShapeError {
            prop: Some("appear_delay".into()),
            message: "`appear_delay` needs an `appear` transition".into(),
        });
    }
    if spec.name == "Slider"
        && shape.props.contains(&"continuous")
        && !shape.props.contains(&"current")
    {
        errors.push(ShapeError {
            prop: Some("continuous".into()),
            message: "`continuous` needs a controlled slider (`current`)".into(),
        });
    }
    if spec.name == "PopoverButton" {
        if shape.props.contains(&"open") && shape.props.contains(&"presented") {
            errors.push(ShapeError {
                prop: None,
                message: "choose `presented` or `open`, not both".into(),
            });
        }
        if shape.props.contains(&"open") && !shape.has_action {
            errors.push(ShapeError {
                prop: None,
                message: "`open` needs an action to commit the new value".into(),
            });
        }
        if shape.has_action && !shape.props.contains(&"open") {
            errors.push(ShapeError {
                prop: None,
                message: "a `PopoverButton` action needs `open`".into(),
            });
        }
    }
    if matches!(spec.name, "Switch" | "SplitButton" | "Picker" | "Slider") {
        let controlled = if matches!(spec.name, "Switch" | "SplitButton") {
            "checked"
        } else if spec.name == "Slider" {
            "current"
        } else {
            "selected"
        };
        if shape.props.contains(&"value") == shape.props.contains(&controlled) {
            errors.push(ShapeError {
                prop: None,
                message: format!(
                    "`{}` needs exactly one of `value` or `{controlled}`",
                    spec.name
                ),
            });
        }
        if shape.props.contains(&controlled) && !shape.has_action {
            errors.push(ShapeError {
                prop: None,
                message: format!("`{controlled}` needs an action to commit the new value"),
            });
        }
    }
    match (spec.action, shape.has_action) {
        (ActionRule::Required, false) => errors.push(ShapeError {
            prop: None,
            message: format!(
                "error[B301]: UI action must bind a registered command; add `=> client.command()` to `{}`",
                spec.name
            ),
        }),
        (ActionRule::None, true) => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take an action", spec.name),
        }),
        _ => {}
    }
    let children = shape.children.as_deref().unwrap_or(&[]);
    match spec.children {
        Children::None if shape.children.is_some() => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` does not take children", spec.name),
        }),
        Children::One if children.len() != 1 => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` takes exactly one child", spec.name),
        }),
        Children::Only(allowed) => {
            for child in children {
                match child {
                    ChildShape::Element(name) if allowed.contains(name) => {}
                    ChildShape::Element(name) => errors.push(ShapeError {
                        prop: None,
                        message: format!(
                            "`{}` cannot contain `{name}`; allowed children: {}",
                            spec.name,
                            allowed.join(", ")
                        ),
                    }),
                    ChildShape::Other => errors.push(ShapeError {
                        prop: None,
                        message: format!(
                            "`{}` only contains {} elements directly",
                            spec.name,
                            allowed.join(", ")
                        ),
                    }),
                }
            }
        }
        _ => {}
    }
    let count = |kind: &str| {
        children
            .iter()
            .filter(|child| matches!(child, ChildShape::Element(name) if *name == kind))
            .count()
    };
    if spec.name == "Sidebar" && count("SidebarFooter") > 1 {
        errors.push(ShapeError {
            prop: None,
            message: "a `Sidebar` has at most one `SidebarFooter`".into(),
        });
    }
    if spec.name == "Sidebar" && count("SidebarHeader") > 1 {
        errors.push(ShapeError {
            prop: None,
            message: "a `Sidebar` has at most one `SidebarHeader`".into(),
        });
    }
    if spec.name == "Toolbar" && count("Search") > 1 {
        errors.push(ShapeError {
            prop: None,
            message: "a `Toolbar` has at most one `Search`".into(),
        });
    }
    if spec.name == "SplitView" {
        let names: Vec<_> = children
            .iter()
            .map(|child| match child {
                ChildShape::Element(name) => *name,
                ChildShape::Other => "",
            })
            .collect();
        if names != ["Sidebar", "Detail"] {
            errors.push(ShapeError {
                prop: None,
                message: "`SplitView` contains exactly `Sidebar { … }` followed by `Detail { … }`"
                    .into(),
            });
        }
    }
    if spec.name == "Video" && shape.props.contains(&"source") == shape.props.contains(&"player") {
        errors.push(ShapeError {
            prop: None,
            message:
                "`Video` takes exactly one of `source:` (a player created for this declaration) \
                      or `player:` (an application player)"
                    .into(),
        });
    }
    if spec.name == "Query" && shape.props.contains(&"icon") && !shape.props.contains(&"empty") {
        errors.push(ShapeError {
            prop: Some("icon".into()),
            message: "`icon:` decorates the `empty:` state; declare `empty:` too".into(),
        });
    }
    if spec.name == "Window" {
        let check = shape.props.contains(&"confirm_close");
        if check != shape.props.contains(&"close_message") {
            errors.push(ShapeError {
                prop: None,
                message: "`confirm_close:` and `close_message:` are declared together".into(),
            });
        }
        if shape.has_action && !check {
            errors.push(ShapeError {
                prop: None,
                message: "a `Window` action is its save command, which needs `confirm_close:`"
                    .into(),
            });
        }
    }
    if spec.name == "Navigation" {
        let destinations = children
            .iter()
            .filter(|child| matches!(child, ChildShape::Element("Destination")))
            .count();
        if destinations != 1 {
            errors.push(ShapeError {
                prop: None,
                message: "`Navigation` contains exactly one `Destination |route| { … }`".into(),
            });
        }
    }
    match spec.placement {
        Placement::Root if !shape.is_root => errors.push(ShapeError {
            prop: None,
            message: format!("`{}` must be the root of a component", spec.name),
        }),
        Placement::Within(parents) if !shape.parent.is_some_and(|p| parents.contains(&p)) => errors
            .push(ShapeError {
                prop: None,
                message: format!(
                    "`{}` may only appear directly inside {}",
                    spec.name,
                    parents.join(" or ")
                ),
            }),
        _ => {}
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape<'a>(name: &'a str, props: Vec<&'a str>, has_action: bool) -> ElementShape<'a> {
        ElementShape {
            name,
            props,
            has_template: false,
            children: None,
            has_action,
            parent: Some("Column"),
            is_root: false,
        }
    }

    #[test]
    fn adaptive_image_ratio_supplies_its_size() {
        assert!(check_shape(&shape("Image", vec!["source", "aspect"], false)).is_empty());
        assert!(!check_shape(&shape("Image", vec!["source"], false)).is_empty());
        assert!(check_shape(&shape("Grid", vec!["min_width", "gap", "row_gap"], false)).is_empty());
    }

    #[test]
    fn buttons_without_actions_fail_with_b301() {
        let errors = check_shape(&shape("Button", vec!["id", "label"], false));
        assert!(errors.iter().any(|error| error.message.contains("B301")));
        assert!(check_shape(&shape("Button", vec!["id", "label"], true)).is_empty());
    }

    #[test]
    fn unknown_components_and_props_are_reported() {
        assert!(!check_shape(&shape("Marquee", vec![], false)).is_empty());
        let errors = check_shape(&shape("Text", vec!["content", "color"], false));
        assert_eq!(errors[0].prop.as_deref(), Some("color"));
    }

    #[test]
    fn interactive_components_require_ids() {
        for spec in COMPONENTS.iter().filter(|spec| spec.interactive) {
            assert!(
                spec.prop("id").is_some_and(|prop| prop.required),
                "{} needs a required id",
                spec.name
            );
        }
    }

    #[test]
    fn video_takes_a_source_or_a_player() {
        for props in [vec!["id"], vec!["id", "source", "player"]] {
            let errors = check_shape(&shape("Video", props, false));
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains("exactly one"))
            );
        }
        assert!(check_shape(&shape("Video", vec!["id", "source"], false)).is_empty());
        assert!(check_shape(&shape("Video", vec!["id", "player"], false)).is_empty());
    }

    #[test]
    fn queries_need_a_call_an_id_and_a_template() {
        let mut query = shape("Query", vec!["id", "query"], false);
        assert!(
            check_shape(&query)
                .iter()
                .any(|error| error.message.contains("template"))
        );
        query.has_template = true;
        assert!(check_shape(&query).is_empty());
        query.props.push("icon");
        assert!(!check_shape(&query).is_empty());
        query.props.push("empty");
        assert!(check_shape(&query).is_empty());
        assert!(!check_shape(&shape("Query", vec!["query"], false)).is_empty());
    }

    #[test]
    fn placement_rules_apply() {
        let mut item = shape("SidebarItem", vec!["value", "label"], false);
        assert!(!check_shape(&item).is_empty());
        item.parent = Some("Section");
        assert!(check_shape(&item).is_empty());
    }
}
