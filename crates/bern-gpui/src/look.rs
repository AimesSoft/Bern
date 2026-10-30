//! `style!` rules for controls: what a button or icon button looks like at
//! rest, under the pointer, pressed and disabled, over the colors of its
//! recipe; and the sizes and shapes of system buttons.

use crate::Color;
use crate::ctx::Ctx;
use crate::layout::color;
use crate::units::px;
use gpui::{BoxShadow, Hsla, Rgba, Styled, hsla, point};

/// One state's look: what it changes; `none` keeps the rest look, so the
/// state gives no feedback.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    /// A selected item filled with the color, its text in the color's
    /// pair where it has one.
    pub fill: Option<Color>,
    /// Color and width.
    pub border: Option<(Color, f32)>,
    /// Blur radius, offset down and opacity.
    pub shadow: Option<(f32, f32, f32)>,
    pub opacity: Option<f32>,
    /// Across and down, drawn about the center.
    pub scale: Option<(f32, f32)>,
    pub none: bool,
}

impl Look {
    pub const NONE: Look = Look {
        background: None,
        foreground: None,
        fill: None,
        border: None,
        shadow: None,
        opacity: None,
        scale: None,
        none: false,
    };
}

/// `style!` sizes of system controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    Small,
    Regular,
    Large,
    ExtraLarge,
}

/// A control's rules for one kind and variant.
#[derive(Clone, Copy, Debug, Default)]
pub struct ControlStyle {
    pub base: Look,
    pub corner: Option<f32>,
    pub padding: Option<f32>,
    pub size: Option<ControlSize>,
    pub hover: Option<Look>,
    pub pressed: Option<Look>,
    pub disabled: Option<Look>,
    pub selected: Option<Look>,
}

impl ControlStyle {
    pub const NONE: ControlStyle = ControlStyle {
        base: Look::NONE,
        corner: None,
        padding: None,
        size: None,
        hover: None,
        pressed: None,
        disabled: None,
        selected: None,
    };
}

/// `shape` of a system control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Default,
    /// Fully round ends.
    Capsule,
    /// A small capsule, as a filter chip.
    Chip,
}

/// Height, horizontal padding and text size of a button.
pub(crate) fn metrics(size: Option<ControlSize>, shape: Shape) -> (f32, f32, f32) {
    match (shape, size) {
        (Shape::Chip, _) => (28.0, 12.0, 13.0),
        (_, Some(ControlSize::Small)) => (28.0, 10.0, 12.5),
        (_, Some(ControlSize::Large)) => (42.0, 20.0, 14.5),
        (_, Some(ControlSize::ExtraLarge)) => (50.0, 24.0, 16.0),
        _ => (36.0, 14.0, 13.5),
    }
}

/// The colors of a control in one state: its recipe's, then the rules'.
#[derive(Clone, Copy)]
pub struct Colors {
    pub background: Rgba,
    pub foreground: Rgba,
    pub border: Option<(Rgba, f32)>,
    pub shadow: Option<(f32, f32, f32)>,
    pub opacity: f32,
}

impl Colors {
    /// A recipe's colors in `state`, with a border where `bordered`.
    pub(crate) fn recipe(c: &Ctx, recipe: &str, state: &str, bordered: bool) -> Colors {
        Colors {
            background: c.slot(recipe, state, "background"),
            foreground: c.slot(recipe, state, "foreground"),
            border: bordered.then(|| (c.slot(recipe, state, "border"), 1.0)),
            shadow: None,
            opacity: 1.0,
        }
    }

    pub(crate) fn with(self, c: &Ctx, look: Option<&Look>) -> Colors {
        let Some(look) = look else {
            return self;
        };
        let background = look.fill.or(look.background);
        let foreground = look.foreground.or(match look.fill {
            Some(Color::Role("accent")) => Some(Color::Role("on_accent")),
            Some(Color::Role("status.error")) => Some(Color::Role("on_error")),
            _ => None,
        });
        Colors {
            background: background.map(|bg| color(c, bg)).unwrap_or(self.background),
            foreground: foreground.map(|fg| color(c, fg)).unwrap_or(self.foreground),
            border: look
                .border
                .map(|(border, width)| (color(c, border), width))
                .or(self.border),
            shadow: look.shadow.or(self.shadow),
            opacity: look.opacity.unwrap_or(self.opacity),
        }
    }
}

pub(crate) fn shadow(shadow: (f32, f32, f32)) -> Vec<BoxShadow> {
    let (radius, y, opacity) = shadow;
    let black: Hsla = hsla(0.0, 0.0, 0.0, opacity);
    vec![BoxShadow {
        color: black,
        offset: point(px(0.0), px(y)),
        blur_radius: px(radius),
        spread_radius: px(0.0),
        inset: false,
    }]
}

/// Paint `colors` on an element at rest.
pub(crate) fn paint<E: Styled>(element: E, colors: Colors) -> E {
    let element = element
        .bg(colors.background)
        .text_color(colors.foreground)
        .opacity(colors.opacity);
    let element = match colors.border {
        Some((border, width)) => element.border(px(width)).border_color(border),
        None => element,
    };
    match colors.shadow {
        Some(lifted) => element.shadow(shadow(lifted)),
        None => element,
    }
}

/// Paint `colors` in a state refinement (hover, pressed).
pub(crate) fn refine(style: gpui::StyleRefinement, colors: Colors) -> gpui::StyleRefinement {
    let style = style
        .bg(colors.background)
        .text_color(colors.foreground)
        .opacity(colors.opacity);
    let style = match colors.border {
        Some((border, _)) => style.border_color(border),
        None => style,
    };
    match colors.shadow {
        Some(lifted) => style.shadow(shadow(lifted)),
        None => style,
    }
}
