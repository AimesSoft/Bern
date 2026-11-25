//! Core box-model, paint, typography, state, and accessibility IR primitives.
//!
//! These types model the intersection of CSS Flexbox/Grid and GPUI's Taffy
//! layout engine so every self-drawn backend maps layout and painting 1:1
//! without pixel-buffer rasterization or loss of platform IME / accessibility.

use bern_theme::Srgb;

/// 8-bit sRGB + alpha color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const BLACK: Self = Self::new(0, 0, 0, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_srgb(color: Srgb, alpha: u8) -> Self {
        Self {
            r: color.r,
            g: color.g,
            b: color.b,
            a: alpha,
        }
    }

    pub fn with_alpha_f32(self, alpha: f32) -> Self {
        let clamped = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self { a: clamped, ..self }
    }

    pub fn to_f32_array(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }

    /// CSS serialization (`#rrggbb` when opaque, `rgb(r g b / a)` when translucent).
    pub fn css(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!(
                "rgb({} {} {} / {:.3})",
                self.r,
                self.g,
                self.b,
                f64::from(self.a) / 255.0
            )
        }
    }
}

/// Semantic or resolved color reference in a [`SurfaceNode`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorValue {
    /// Transparent / unpainted.
    Transparent,
    /// Named semantic role resolved through [`crate::SurfacePalette`], e.g.
    /// `"accent"`, `"surface.raised"`, `"sidebar.surface"`, `"slider.thumb"`.
    Role(&'static str),
    /// Named semantic role with an explicit alpha multiplier in `0.0..=1.0`,
    /// used for tinted semantic pills and banners.
    RoleAlpha(&'static str, f32),
    /// Recipe slot lookup (`recipe`, `state`, `slot`).
    Recipe {
        recipe: &'static str,
        state: &'static str,
        slot: &'static str,
    },
    /// Fixed appearance-dependent sRGB pair (from `style!`).
    Fixed { light: Rgba8, dark: Rgba8 },
    /// Literal resolved RGBA value.
    Rgba(Rgba8),
}

/// Dimension length in logical points or parent fraction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    #[default]
    Auto,
    Px(f32),
    /// Fraction of the parent box (`0.0..=1.0`).
    Fraction(f32),
    /// Full parent extent (`100%`).
    Full,
}

/// Four-sided spacing (padding or margin) in logical points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }

    pub const fn xy(x: f32, y: f32) -> Self {
        Self {
            top: y,
            right: x,
            bottom: y,
            left: x,
        }
    }

    pub const fn trbl(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub fn is_zero(self) -> bool {
        self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0 && self.left == 0.0
    }
}

/// Corner radii in logical points (`tl`, `tr`, `br`, `bl`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corners {
    pub tl: f32,
    pub tr: f32,
    pub br: f32,
    pub bl: f32,
}

impl Corners {
    pub const ZERO: Self = Self::all(0.0);
    /// Full pill/capsule rounding (`999px`).
    pub const PILL: Self = Self::all(999.0);

    pub const fn all(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }

    pub const fn top(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: 0.0,
            bl: 0.0,
        }
    }

    pub const fn is_pill(self) -> bool {
        self.tl >= 999.0 && self.tr >= 999.0 && self.br >= 999.0 && self.bl >= 999.0
    }

    pub fn is_uniform(self) -> bool {
        (self.tl - self.tr).abs() < 0.01
            && (self.tr - self.br).abs() < 0.01
            && (self.br - self.bl).abs() < 0.01
    }
}

/// Which edges of a box receive a border stroke.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BorderSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl BorderSides {
    pub const ALL: Self = Self {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };
    pub const TOP: Self = Self {
        top: true,
        right: false,
        bottom: false,
        left: false,
    };
    pub const RIGHT: Self = Self {
        top: false,
        right: true,
        bottom: false,
        left: false,
    };
    pub const BOTTOM: Self = Self {
        top: false,
        right: false,
        bottom: true,
        left: false,
    };
    pub const LEFT: Self = Self {
        top: false,
        right: false,
        bottom: false,
        left: true,
    };
    pub const SHEET_TOP: Self = Self {
        top: true,
        right: true,
        bottom: false,
        left: true,
    };
}

/// Box border stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub width: f32,
    pub color: ColorValue,
    pub sides: BorderSides,
}

impl Border {
    pub const fn all(width: f32, color: ColorValue) -> Self {
        Self {
            width,
            color,
            sides: BorderSides::ALL,
        }
    }

    pub const fn sides(width: f32, color: ColorValue, sides: BorderSides) -> Self {
        Self {
            width,
            color,
            sides,
        }
    }
}

/// Drop or inset box shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: ColorValue,
    pub inset: bool,
}

impl Shadow {
    pub const fn drop(offset_y: f32, blur: f32, color: ColorValue) -> Self {
        Self {
            offset_x: 0.0,
            offset_y,
            blur,
            spread: 0.0,
            color,
            inset: false,
        }
    }
}

/// Typography specification in logical points and CSS numeric font weights.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub size: f32,
    pub line_height: Option<f32>,
    pub weight: u16,
    pub monospace: bool,
    pub tabular_nums: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            size: 14.0,
            line_height: Some(20.0),
            weight: 400,
            monospace: false,
            tabular_nums: false,
        }
    }
}

/// Layout orientation axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    Horizontal,
    Vertical,
}

/// Layout container mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    #[default]
    Flex,
    Stack,
    Grid,
}

/// Cross-axis alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
    Baseline,
}

/// Main-axis justification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Box positioning scheme.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Position {
    #[default]
    Relative,
    Absolute {
        top: Option<Length>,
        right: Option<Length>,
        bottom: Option<Length>,
        left: Option<Length>,
    },
}

/// Overflow clipping / scrolling policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    ScrollY,
    ScrollX,
    ScrollBoth,
}

/// Pointer cursor hint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cursor {
    #[default]
    Default,
    Pointer,
    Text,
    ColResize,
    RowResize,
    Grab,
}

/// Semantic accessibility role (maps directly to WAI-ARIA and AccessKit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AriaRole {
    Button,
    Switch,
    Checkbox,
    Slider,
    Textbox,
    Searchbox,
    Combobox,
    Listbox,
    Option,
    Menu,
    MenuItem,
    TabList,
    Tab,
    TabPanel,
    Table,
    Tree,
    TreeItem,
    Dialog,
    AlertDialog,
    Status,
    Navigation,
    Region,
    Link,
    Tooltip,
    Separator,
    ProgressBar,
    Img,
    Group,
}

impl AriaRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            AriaRole::Button => "button",
            AriaRole::Switch => "switch",
            AriaRole::Checkbox => "checkbox",
            AriaRole::Slider => "slider",
            AriaRole::Textbox => "textbox",
            AriaRole::Searchbox => "searchbox",
            AriaRole::Combobox => "combobox",
            AriaRole::Listbox => "listbox",
            AriaRole::Option => "option",
            AriaRole::Menu => "menu",
            AriaRole::MenuItem => "menuitem",
            AriaRole::TabList => "tablist",
            AriaRole::Tab => "tab",
            AriaRole::TabPanel => "tabpanel",
            AriaRole::Table => "table",
            AriaRole::Tree => "tree",
            AriaRole::TreeItem => "treeitem",
            AriaRole::Dialog => "dialog",
            AriaRole::AlertDialog => "alertdialog",
            AriaRole::Status => "status",
            AriaRole::Navigation => "navigation",
            AriaRole::Region => "region",
            AriaRole::Link => "link",
            AriaRole::Tooltip => "tooltip",
            AriaRole::Separator => "separator",
            AriaRole::ProgressBar => "progressbar",
            AriaRole::Img => "img",
            AriaRole::Group => "group",
        }
    }
}

/// Named host/child mounting slot inside a lowered control tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlotId(pub &'static str);

/// Box-model layout properties.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub display: Display,
    pub direction: Axis,
    pub wrap: bool,
    pub align: Align,
    pub justify: Justify,
    pub gap: f32,
    pub padding: Edges,
    pub margin: Edges,
    pub width: Length,
    pub height: Length,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub min_height: Option<Length>,
    pub max_height: Option<Length>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub position: Position,
    pub overflow: Overflow,
}

impl Layout {
    pub fn row() -> Self {
        Self {
            direction: Axis::Horizontal,
            align: Align::Center,
            ..Self::default()
        }
    }

    pub fn column() -> Self {
        Self {
            direction: Axis::Vertical,
            ..Self::default()
        }
    }

    pub fn centered() -> Self {
        Self {
            align: Align::Center,
            justify: Justify::Center,
            ..Self::default()
        }
    }
}

/// Visual paint and typography properties of a box.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualStyle {
    pub background: Option<ColorValue>,
    pub foreground: Option<ColorValue>,
    pub border: Option<Border>,
    pub corners: Option<Corners>,
    pub shadows: Vec<Shadow>,
    pub opacity: Option<f32>,
    pub scale: Option<(f32, f32)>,
    pub cursor: Option<Cursor>,
    pub typography: Option<Typography>,
}

/// Interactive state style refinements (hover, pressed, selected, disabled, focus).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateOverrides {
    pub hover: Option<VisualStyle>,
    pub pressed: Option<VisualStyle>,
    pub selected: Option<VisualStyle>,
    pub disabled: Option<VisualStyle>,
    pub focus_ring: Option<ColorValue>,
}

/// Container box node in the Surface IR.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoxNode {
    /// Canonical part/selector name (e.g. `"bern-slider"`, `"bern-sidebar"`).
    pub part: &'static str,
    pub role: Option<AriaRole>,
    pub layout: Layout,
    pub style: VisualStyle,
    pub states: StateOverrides,
    pub children: Vec<SurfaceNode>,
}

impl BoxNode {
    pub fn new(part: &'static str) -> Self {
        Self {
            part,
            ..Self::default()
        }
    }

    pub fn with_role(mut self, role: AriaRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    pub fn with_style(mut self, style: VisualStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_states(mut self, states: StateOverrides) -> Self {
        self.states = states;
        self
    }

    pub fn child(mut self, node: SurfaceNode) -> Self {
        self.children.push(node);
        self
    }

    pub fn children(mut self, nodes: impl IntoIterator<Item = SurfaceNode>) -> Self {
        self.children.extend(nodes);
        self
    }

    pub fn into_node(self) -> SurfaceNode {
        SurfaceNode::Box(self)
    }
}

/// Text leaf node.
#[derive(Clone, Debug, PartialEq)]
pub struct TextNode {
    pub content: String,
    pub typography: Typography,
    pub color: ColorValue,
    pub lines: Option<usize>,
    pub truncate: bool,
    pub selectable: bool,
}

/// Vector icon leaf node (SF Symbols name from Bern's icon catalog).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconNode {
    pub name: &'static str,
    pub size: f32,
    pub color: ColorValue,
}

/// Geometric linear track primitive used by `Slider`, `Progress`, and media scrubbers.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackNode {
    pub part: &'static str,
    pub axis: Axis,
    /// Normalized filled fraction in `0.0..=1.0`.
    pub fraction: f32,
    /// Interactive cross-axis hit target extent in points.
    pub hit_extent: f32,
    /// Minimum main-axis extent (used for vertical sliders).
    pub min_main_extent: Option<f32>,
    /// Visual rail thickness in points.
    pub thickness: f32,
    /// Corner radius of rail and active fill.
    pub radius: f32,
    pub track_color: ColorValue,
    pub fill_color: ColorValue,
    pub disabled: bool,
    /// Optional draggable thumb box (e.g., white vertical pill on horizontal slider).
    pub thumb: Option<BoxNode>,
}

/// Circular progress ring primitive used by `Progress(style: ring)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RingNode {
    pub fraction: f32,
    pub diameter: f32,
    pub stroke_width: f32,
    pub color: ColorValue,
}

/// One node in a lowered self-drawn surface tree.
#[derive(Clone, Debug, PartialEq)]
pub enum SurfaceNode {
    Box(BoxNode),
    Text(TextNode),
    Icon(IconNode),
    Track(TrackNode),
    Ring(RingNode),
    Spacer,
    /// Dynamic child or platform-input mount point (`"content"`, `"input"`, `"actions"`, ...).
    Slot(SlotId),
}

/// Root document of a lowered self-drawn control or view.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceTree {
    pub ir_version: u32,
    pub root: SurfaceNode,
}

impl SurfaceTree {
    pub fn new(root: SurfaceNode) -> Self {
        Self {
            ir_version: crate::IR_VERSION,
            root,
        }
    }
}
