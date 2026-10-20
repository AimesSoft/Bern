//! Lower `style!` into the application's style sheet and check it: known
//! controls, properties and states, semantic color roles, explicit colors
//! for both modes, and motion tokens that exist.

use crate::check::location;
use bern_ir::style::{
    AccentDef, BUILT_IN_MOTION, Band, COLOR_ROLES, ColorRef, ControlSize, CornerDef, Edge, Metric,
    MotionToken, PatternDef, StateStyle, StyleEffect, StyleRule, StyleSheet, StyleState,
    TransitionDef, TransitionKind, built_in_transitions,
};
use bern_ir::{SourceLocation, Srgb};
use bern_syntax::{StyleArg, StyleItem, StyleValue};
use bern_target_api::Diagnostic;
use proc_macro2::{Ident, Span};

/// Platforms a rule may name in `#[target(…)]` and `#[exempt(…)]`.
pub const PLATFORMS: [&str; 7] = [
    "macos", "ios", "tvos", "windows", "linux", "android", "browser",
];

/// Controls that take style overrides, with the states that apply to them.
const STYLEABLE: &[(&str, &[StyleState])] = &[
    ("Stack", &[]),
    // A row's or column's lengths, per platform.
    ("Row", &[]),
    ("Column", &[]),
    // A form's `background` is its rows'; the form itself shows the page.
    ("Form", &[]),
    (
        "Button",
        &[
            StyleState::Hover,
            StyleState::Pressed,
            StyleState::Focus,
            StyleState::Disabled,
        ],
    ),
    (
        "IconButton",
        &[
            StyleState::Hover,
            StyleState::Pressed,
            StyleState::Focus,
            StyleState::Disabled,
        ],
    ),
    (
        "SidebarItem",
        &[
            StyleState::Hover,
            StyleState::Pressed,
            StyleState::Selected,
            StyleState::Focus,
        ],
    ),
];

/// Effects allowed outside any state, and those allowed inside one.
const BASE_EFFECTS: &[&str] = &[
    "background",
    "foreground",
    "border",
    "corner",
    "padding",
    "size",
    "shadow",
    "gap",
    "padding_x",
    "padding_y",
    "width",
    "height",
    "font_size",
    "row_height",
];

/// The elements that write each length a style may set in their place.
fn writes(metric: Metric) -> &'static [&'static str] {
    match metric {
        Metric::Gap | Metric::PaddingX | Metric::PaddingY => &["Row", "Column", "Form"],
        Metric::RowHeight => &["Form"],
        Metric::Width => &["Row", "Column", "Stack"],
        Metric::Height => &["Row", "Column", "Stack", "IconButton"],
        Metric::FontSize => &["IconButton"],
    }
}
const STATE_EFFECTS: &[&str] = &[
    "background",
    "foreground",
    "border",
    "scale",
    "offset",
    "opacity",
    "shadow",
    "system",
    "none",
];
/// Only the selected state fills or marks an item.
const SELECTED_EFFECTS: &[&str] = &["fill", "indicator"];

struct Lowering<'a> {
    file: &'a str,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Lowering<'_> {
    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::error(message.into()).at(location(self.file, span)));
    }

    fn location(&self, span: Span) -> SourceLocation {
        location(self.file, span)
    }

    fn number(&mut self, value: &StyleValue, what: &str) -> Option<f64> {
        match value {
            StyleValue::Number(number, _) => Some(*number),
            other => {
                self.error(other.span(), format!("{what} is a number"));
                None
            }
        }
    }

    /// A length of 0 or more, such as a width or a radius.
    fn length(&mut self, value: &StyleValue, what: &str) -> Option<f64> {
        let length = self.number(value, what)?;
        if length < 0.0 {
            self.error(value.span(), format!("{what} is 0 or more"));
            return None;
        }
        Some(length)
    }

    fn edge(&mut self, value: &StyleValue) -> Option<Edge> {
        match value.dotted().as_deref() {
            Some("leading") => Some(Edge::Leading),
            Some("trailing") => Some(Edge::Trailing),
            Some("top") => Some(Edge::Top),
            Some("bottom") => Some(Edge::Bottom),
            Some(other) => {
                self.error(
                    value.span(),
                    format!("`{other}` is not an edge; use leading, trailing, top or bottom"),
                );
                None
            }
            None => {
                self.error(value.span(), "an edge is leading, trailing, top or bottom");
                None
            }
        }
    }

    fn color(&mut self, value: &StyleValue) -> Option<ColorRef> {
        match value {
            StyleValue::Name(_) => {
                let role = value.dotted().unwrap_or_default();
                if COLOR_ROLES.contains(&role.as_str()) {
                    Some(ColorRef::Role { role })
                } else {
                    self.error(
                        value.span(),
                        format!(
                            "`{role}` is not a color role; use one of {}",
                            COLOR_ROLES.join(", ")
                        ),
                    );
                    None
                }
            }
            StyleValue::Call { name, args } if name == "color" => {
                let mut light = None;
                let mut dark = None;
                for arg in args {
                    let slot = match arg.name.as_ref().map(ToString::to_string).as_deref() {
                        Some("light") => &mut light,
                        Some("dark") => &mut dark,
                        _ => {
                            self.error(
                                name.span(),
                                "write `color(light: \"#RRGGBB\", dark: \"#RRGGBB\")`",
                            );
                            return None;
                        }
                    };
                    match &arg.value {
                        StyleValue::Text(hex, span) => match Srgb::parse(hex) {
                            Some(color) => *slot = Some(color),
                            None => self.error(*span, format!("`{hex}` is not a `#RRGGBB` color")),
                        },
                        other => self.error(other.span(), "a color is written `\"#RRGGBB\"`"),
                    }
                }
                match (light, dark) {
                    (Some(light), Some(dark)) => Some(ColorRef::Fixed { light, dark }),
                    _ => {
                        self.error(
                            name.span(),
                            "a fixed color gives both modes: `color(light: \"#…\", dark: \"#…\")`",
                        );
                        None
                    }
                }
            }
            other => {
                self.error(
                    other.span(),
                    "a color is a role such as `accent` or `color(light: …, dark: …)`",
                );
                None
            }
        }
    }

    /// The trailing motion token of a call such as `scale(1.03, snappy)`.
    fn motion(&mut self, args: &[StyleArg], sheet: &StyleSheet) -> String {
        let named = args
            .iter()
            .rev()
            .find(|arg| arg.name.is_none() && matches!(arg.value, StyleValue::Name(_)));
        match named.and_then(|arg| arg.value.dotted()) {
            Some(token) if sheet.token(&token).is_some() => token,
            Some(token) => {
                let span = named
                    .map(|arg| arg.value.span())
                    .unwrap_or_else(Span::call_site);
                self.error(
                    span,
                    format!("`{token}` is not a motion token; declare it under `motion {{ … }}`"),
                );
                "smooth".into()
            }
            None => "smooth".into(),
        }
    }

    fn numbers(&mut self, args: &[StyleArg]) -> (Vec<f64>, Vec<(String, f64)>) {
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for arg in args {
            if let StyleValue::Number(number, _) = arg.value {
                match &arg.name {
                    Some(name) => named.push((name.to_string(), number)),
                    None => positional.push(number),
                }
            }
        }
        (positional, named)
    }

    /// `slide` or `grow`, with the argument that names its motion token.
    /// The edge and a slide's distance are given by name,
    /// `slide(edge: top, distance: 24, snappy)`, or by position,
    /// `slide(top, 24, snappy)`: the edge is the name before the token, and
    /// a name alone is the edge when it is one.
    fn edge_transition<'a>(
        &mut self,
        name: &Ident,
        args: &'a [StyleArg],
        sheet: &StyleSheet,
    ) -> (TransitionKind, Option<&'a StyleArg>) {
        let slide = name == "slide";
        let usage = if slide {
            "write `slide(bottom, 16, token)` or `slide(edge: bottom, distance: 16, token)`"
        } else {
            "write `grow(bottom, token)` or `grow(edge: bottom, token)`"
        };
        let mut edge = None;
        let mut distance = None;
        let mut names = Vec::new();
        for arg in args {
            match (
                arg.name.as_ref().map(ToString::to_string).as_deref(),
                &arg.value,
            ) {
                (Some("edge"), StyleValue::Name(_)) if edge.is_none() => edge = Some(&arg.value),
                (Some("distance") | None, StyleValue::Number(..))
                    if slide && distance.is_none() =>
                {
                    distance = Some(&arg.value)
                }
                (None, StyleValue::Name(_)) => names.push(arg),
                _ => self.error(arg.value.span(), usage),
            }
        }
        let is_edge = |arg: &StyleArg| {
            arg.value
                .dotted()
                .is_some_and(|word| ["leading", "trailing", "top", "bottom"].contains(&&*word))
        };
        let token = match names.as_slice() {
            [] => None,
            [only] if edge.is_none() && is_edge(only) => {
                edge = Some(&only.value);
                None
            }
            [token] if edge.is_none() => {
                let word = token.value.dotted().unwrap_or_default();
                if sheet.token(&word).is_some() {
                    Some(*token)
                } else {
                    self.error(
                        token.value.span(),
                        format!(
                            "`{word}` is neither an edge nor a motion token; an edge is leading, trailing, top or bottom"
                        ),
                    );
                    None
                }
            }
            [token] => Some(*token),
            [first, token] if edge.is_none() => {
                edge = Some(&first.value);
                Some(*token)
            }
            _ => {
                self.error(name.span(), usage);
                None
            }
        };
        let edge = edge
            .and_then(|value| self.edge(value))
            .unwrap_or(Edge::Bottom);
        let kind = if slide {
            TransitionKind::Slide {
                edge,
                distance: distance
                    .and_then(|value| self.length(value, "a slide's distance"))
                    .unwrap_or(16.0),
            }
        } else {
            TransitionKind::Grow { edge }
        };
        (kind, token)
    }

    fn effect(&mut self, value: &StyleValue, sheet: &StyleSheet) -> Option<StyleEffect> {
        match value {
            StyleValue::Name(path) if path.len() == 1 && path[0] == "system" => {
                Some(StyleEffect::System)
            }
            StyleValue::Name(path) if path.len() == 1 && path[0] == "none" => {
                Some(StyleEffect::Nothing)
            }
            StyleValue::Call { name, args } => {
                let kind = name.to_string();
                // The color is the first argument without a name.
                let first = args
                    .iter()
                    .find(|arg| arg.name.is_none())
                    .map(|arg| &arg.value);
                if first.is_none()
                    && ["background", "foreground", "fill", "border", "indicator"]
                        .contains(&kind.as_str())
                {
                    self.error(
                        name.span(),
                        format!("`{kind}` takes a color, such as `{kind}(accent)`"),
                    );
                    return None;
                }
                let (positional, named) = self.numbers(args);
                let get = |key: &str| {
                    named
                        .iter()
                        .find(|(name, _)| name == key)
                        .map(|(_, value)| *value)
                };
                let width = |lowering: &mut Self, default: f64, what: &str| {
                    let width = get("width").unwrap_or(default);
                    if width < 0.0 {
                        lowering.error(name.span(), format!("{what} is 0 or more"));
                        return None;
                    }
                    Some(width)
                };
                match kind.as_str() {
                    "background" => Some(StyleEffect::Background {
                        color: self.color(first?)?,
                    }),
                    "foreground" => Some(StyleEffect::Foreground {
                        color: self.color(first?)?,
                    }),
                    "fill" => Some(StyleEffect::Fill {
                        color: self.color(first?)?,
                    }),
                    "border" => Some(StyleEffect::Border {
                        color: self.color(first?)?,
                        width: width(self, 1.0, "a border's width")?,
                    }),
                    "indicator" => {
                        let edge = match args
                            .iter()
                            .find(|arg| arg.name.as_ref().is_some_and(|name| name == "edge"))
                        {
                            Some(arg) => self.edge(&arg.value).unwrap_or(Edge::Leading),
                            None => Edge::Leading,
                        };
                        Some(StyleEffect::Indicator {
                            color: self.color(first?)?,
                            edge,
                            width: width(self, 3.0, "an indicator's width")?,
                        })
                    }
                    "scale" => {
                        let uniform = positional.first().copied();
                        let x = get("x").or(uniform);
                        let y = get("y").or(uniform);
                        let (Some(x), Some(y)) = (x, y) else {
                            self.error(
                                name.span(),
                                "write `scale(1.03, token)` or `scale(x: 0.97, y: 0.9, token)`",
                            );
                            return None;
                        };
                        Some(StyleEffect::Scale {
                            x,
                            y,
                            motion: self.motion(args, sheet),
                        })
                    }
                    "offset" => Some(StyleEffect::Offset {
                        x: get("x").unwrap_or(0.0),
                        y: get("y").unwrap_or(0.0),
                        motion: self.motion(args, sheet),
                    }),
                    "opacity" => {
                        let Some(value) = positional.first().copied() else {
                            self.error(name.span(), "write `opacity(0.6, token)`");
                            return None;
                        };
                        if !(0.0..=1.0).contains(&value) {
                            self.error(name.span(), "an opacity is from 0 to 1");
                            return None;
                        }
                        Some(StyleEffect::Opacity {
                            value,
                            motion: self.motion(args, sheet),
                        })
                    }
                    "shadow" => {
                        let Some(radius) = positional.first().copied() else {
                            self.error(
                                name.span(),
                                "write `shadow(12)` or `shadow(12, y: 4, opacity: 0.2)`",
                            );
                            return None;
                        };
                        let opacity = get("opacity").unwrap_or(0.2);
                        if radius < 0.0 || !(0.0..=1.0).contains(&opacity) {
                            self.error(
                                name.span(),
                                "a shadow has a radius of 0 or more and an opacity from 0 to 1",
                            );
                            return None;
                        }
                        Some(StyleEffect::Shadow {
                            radius,
                            y: get("y").unwrap_or(radius / 3.0),
                            opacity,
                        })
                    }
                    other => {
                        self.error(name.span(), format!("`{other}` is not a style effect"));
                        None
                    }
                }
            }
            other => {
                self.error(other.span(), "expected an effect such as `scale(1.03, snappy)`, `fill(accent)`, `system` or `none`");
                None
            }
        }
    }
}

/// Lower every `style!` of the application. `items` pairs each with the
/// file declaring it.
pub fn lower(items: &[(String, StyleItem)], diagnostics: &mut Vec<Diagnostic>) -> StyleSheet {
    let mut sheet = StyleSheet {
        motion: BUILT_IN_MOTION
            .iter()
            .map(|(name, period, damping)| MotionToken {
                name: (*name).into(),
                period: *period,
                damping: *damping,
            })
            .collect(),
        rules: Vec::new(),
        transitions: built_in_transitions(),
        patterns: Vec::new(),
        accents: Vec::new(),
        text_weights: Vec::new(),
        corners: Vec::new(),
    };
    if items.len() > 1 {
        for (file, item) in &items[1..] {
            let mut lowering = Lowering { file, diagnostics };
            lowering.error(item.span, "an application declares at most one `style!`");
        }
    }
    let Some((file, item)) = items.first() else {
        return sheet;
    };
    let mut lowering = Lowering { file, diagnostics };
    lower_corners(&mut lowering, item, &mut sheet);
    for decl in &item.motion {
        let StyleValue::Call { name, args } = &decl.value else {
            lowering.error(
                decl.value.span(),
                "a motion token is `spring(period: T, damping: ζ)`",
            );
            continue;
        };
        if name != "spring" {
            lowering.error(
                name.span(),
                "a motion token is `spring(period: T, damping: ζ)`",
            );
            continue;
        }
        let (_, named) = lowering.numbers(args);
        let get = |key: &str| {
            named
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| *value)
        };
        let (Some(period), Some(damping)) = (get("period"), get("damping")) else {
            lowering.error(name.span(), "a spring needs `period:` and `damping:`");
            continue;
        };
        if !(period > 0.0 && damping > 0.0) {
            lowering.error(name.span(), "a spring's period and damping are positive");
            continue;
        }
        let token = MotionToken {
            name: decl.name.to_string(),
            period,
            damping,
        };
        match sheet
            .motion
            .iter_mut()
            .find(|existing| existing.name == token.name)
        {
            Some(existing) => *existing = token,
            None => sheet.motion.push(token),
        }
    }
    for decl in &item.transitions {
        let StyleValue::Call { name, args } = &decl.value else {
            lowering.error(decl.value.span(), "a transition is `fade(token)`, `pop(from: 0.8, token)` or `slide(edge: bottom, distance: 16, token)`");
            continue;
        };
        let (positional, named) = lowering.numbers(args);
        let get = |key: &str| {
            named
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| *value)
        };
        let mut motion = args.as_slice();
        let kind = match name.to_string().as_str() {
            "fade" => TransitionKind::Fade,
            "pop" => TransitionKind::Pop {
                from: get("from").or(positional.first().copied()).unwrap_or(0.8),
            },
            "slide" | "grow" => {
                let (kind, token) = lowering.edge_transition(name, args, &sheet);
                motion = token.map(std::slice::from_ref).unwrap_or_default();
                kind
            }
            other => {
                lowering.error(
                    name.span(),
                    format!("`{other}` is not a transition; use fade, pop, grow or slide"),
                );
                continue;
            }
        };
        let transition = TransitionDef {
            name: decl.name.to_string(),
            kind,
            motion: lowering.motion(motion, &sheet),
        };
        match sheet
            .transitions
            .iter_mut()
            .find(|existing| existing.name == transition.name)
        {
            Some(existing) => *existing = transition,
            None => sheet.transitions.push(transition),
        }
    }
    for decl in &item.patterns {
        let StyleValue::Call { name, args } = &decl.value else {
            lowering.error(
                decl.value.span(),
                "a pattern is `check(band(color, width, opacity), gap(width), …, twill: true)`",
            );
            continue;
        };
        if name != "check" {
            lowering.error(name.span(), "a pattern is `check(band(…), gap(…), …)`");
            continue;
        }
        let mut pattern = PatternDef {
            name: decl.name.to_string(),
            bands: Vec::new(),
            twill: false,
        };
        for arg in args {
            match (&arg.name, &arg.value) {
                (Some(key), StyleValue::Name(path)) if key == "twill" => {
                    pattern.twill = path.len() == 1 && path[0] == "true";
                }
                (
                    None,
                    StyleValue::Call {
                        name: kind,
                        args: band,
                    },
                ) if kind == "band" => {
                    let (Some(color), Some(width)) = (band.first(), band.get(1)) else {
                        lowering.error(kind.span(), "a band is `band(color, width, opacity)`");
                        continue;
                    };
                    let Some(color) = lowering.color(&color.value) else {
                        continue;
                    };
                    let Some(width) = lowering.length(&width.value, "a band's width") else {
                        continue;
                    };
                    let opacity = match band.get(2) {
                        Some(opacity) => {
                            match lowering.number(&opacity.value, "a band's opacity") {
                                Some(value) if !(0.0..=1.0).contains(&value) => {
                                    lowering.error(
                                        opacity.value.span(),
                                        "a band's opacity is from 0 to 1",
                                    );
                                    continue;
                                }
                                value => value.unwrap_or(1.0),
                            }
                        }
                        None => 1.0,
                    };
                    pattern.bands.push(Band {
                        color: Some(color),
                        width,
                        opacity,
                    });
                }
                (
                    None,
                    StyleValue::Call {
                        name: kind,
                        args: gap,
                    },
                ) if kind == "gap" => {
                    let Some(width) = gap.first() else {
                        lowering.error(kind.span(), "a gap is `gap(width)`");
                        continue;
                    };
                    let Some(width) = lowering.length(&width.value, "a gap's width") else {
                        continue;
                    };
                    pattern.bands.push(Band {
                        color: None,
                        width,
                        opacity: 0.0,
                    });
                }
                (_, other) => lowering.error(
                    other.span(),
                    "a pattern holds `band(…)`, `gap(…)` and `twill: true`",
                ),
            }
        }
        if !pattern
            .bands
            .iter()
            .any(|band| band.color.is_some() && band.width > 0.0)
        {
            lowering.error(
                name.span(),
                "a pattern needs a colored band wider than zero",
            );
            continue;
        }
        match sheet
            .patterns
            .iter_mut()
            .find(|existing| existing.name == pattern.name)
        {
            Some(existing) => *existing = pattern,
            None => sheet.patterns.push(pattern),
        }
    }
    for decl in &item.accents {
        let color = match &decl.value {
            StyleValue::Text(hex, span) => match Srgb::parse(hex) {
                Some(color) => Some(ColorRef::Fixed {
                    light: color,
                    dark: color,
                }),
                None => {
                    lowering.error(*span, format!("`{hex}` is not a `#RRGGBB` color"));
                    None
                }
            },
            value @ StyleValue::Call { .. } => lowering.color(value),
            other => {
                lowering.error(
                    other.span(),
                    "an accent is `\"#RRGGBB\"` or `color(light: \"#…\", dark: \"#…\")`",
                );
                None
            }
        };
        let Some(color) = color else { continue };
        if matches!(color, ColorRef::Role { .. }) {
            lowering.error(decl.value.span(), "an accent is a color, not a role");
            continue;
        }
        let accent = AccentDef {
            name: decl.name.to_string(),
            color,
        };
        match sheet
            .accents
            .iter_mut()
            .find(|existing| existing.name == accent.name)
        {
            Some(existing) => *existing = accent,
            None => sheet.accents.push(accent),
        }
    }
    for decl in &item.text {
        let style = decl.name.to_string();
        if style == "control" {
            lowering.error(
                decl.name.span(),
                "`control` is the platform's control font, as its buttons have it; it takes no size or weight of the application's",
            );
            continue;
        }
        if !bern_ir::spec::TEXT_STYLES.contains(&style.as_str()) {
            lowering.error(
                decl.name.span(),
                format!(
                    "`{style}` is not a text style; text styles: {}",
                    bern_ir::spec::TEXT_STYLES.join(", ")
                ),
            );
            continue;
        }
        // `heavy`, or `font(size: 30, weight: heavy)`, with `tv: 34` for
        // the size on a television.
        let (weight, size, tv_size) = match &decl.value {
            StyleValue::Name(path) if path.len() == 1 => (path[0].to_string(), None, None),
            StyleValue::Call { name, args } if name == "font" => {
                let mut weight = None;
                let mut size = None;
                let mut tv_size = None;
                for arg in args {
                    match (
                        arg.name.as_ref().map(|name| name.to_string()).as_deref(),
                        &arg.value,
                    ) {
                        (Some("weight"), StyleValue::Name(path)) if path.len() == 1 => {
                            weight = Some(path[0].to_string())
                        }
                        (Some(name @ ("size" | "tv")), StyleValue::Number(value, span)) => {
                            if *value <= 0.0 {
                                lowering.error(*span, "a text size is more than 0 points");
                            } else if name == "tv" {
                                tv_size = Some(*value);
                            } else {
                                size = Some(*value);
                            }
                        }
                        _ => lowering.error(
                            arg.value.span(),
                            "`font` takes `size: points`, `tv: points` (the size on a television) and `weight: name`",
                        ),
                    }
                }
                (weight.unwrap_or_else(|| "regular".into()), size, tv_size)
            }
            other => {
                lowering.error(
                    other.span(),
                    "a text style takes a weight, such as `heavy`, or `font(size: 30, weight: heavy)`",
                );
                continue;
            }
        };
        if !bern_ir::style::TEXT_WEIGHTS.contains(&weight.as_str()) {
            lowering.error(
                decl.value.span(),
                format!(
                    "`{weight}` is not a weight; weights: {}",
                    bern_ir::style::TEXT_WEIGHTS.join(", ")
                ),
            );
            continue;
        }
        let entry = bern_ir::style::TextWeight {
            style,
            weight,
            size,
            tv_size,
        };
        match sheet
            .text_weights
            .iter_mut()
            .find(|existing| existing.style == entry.style)
        {
            Some(existing) => *existing = entry,
            None => sheet.text_weights.push(entry),
        }
    }
    for rule in &item.rules {
        let component = rule.component.to_string();
        let Some((_, states)) = STYLEABLE.iter().find(|(name, _)| *name == component) else {
            lowering.error(
                rule.component.span(),
                format!(
                    "`{component}` does not take style overrides yet; styleable: {}",
                    STYLEABLE
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
            continue;
        };
        let mut targets = Vec::new();
        for target in &rule.targets {
            let name = target.to_string();
            if PLATFORMS.contains(&name.as_str()) {
                targets.push(name);
            } else {
                lowering.error(
                    target.span(),
                    format!("`{name}` is not a platform; use {}", PLATFORMS.join(", ")),
                );
            }
        }
        let mut exempt = Vec::new();
        for (target, _) in &rule.exempt {
            let name = target.to_string();
            if PLATFORMS.contains(&name.as_str()) {
                exempt.push(name);
            } else {
                lowering.error(
                    target.span(),
                    format!("`{name}` is not a platform; use {}", PLATFORMS.join(", ")),
                );
            }
        }
        let mut lowered = StyleRule {
            component: component.clone(),
            variant: rule.variant.as_ref().map(ToString::to_string),
            targets,
            exempt,
            base: Vec::new(),
            states: Vec::new(),
            settle: None,
            source: lowering.location(rule.span),
        };
        // Named corners the rule takes, by the platforms where they differ.
        let mut corner_names: Vec<String> = Vec::new();
        for prop in &rule.props {
            let name = prop.name.to_string();
            if name == "settle" {
                match prop.value.dotted() {
                    Some(token) if sheet.token(&token).is_some() => lowered.settle = Some(token),
                    _ => lowering.error(prop.value.span(), "`settle` names a motion token"),
                }
                continue;
            }
            if let Some(state) = StyleState::ALL.iter().find(|state| state.name() == name) {
                if !states.contains(state) {
                    lowering.error(
                        prop.name.span(),
                        format!("`{component}` has no `{name}` state"),
                    );
                    continue;
                }
                let mut effects = Vec::new();
                for term in prop.value.terms() {
                    let Some(effect) = lowering.effect(term, &sheet) else {
                        continue;
                    };
                    let allowed = STATE_EFFECTS.contains(&effect.name())
                        || (*state == StyleState::Selected
                            && SELECTED_EFFECTS.contains(&effect.name()));
                    if allowed {
                        effects.push(effect);
                    } else {
                        lowering.error(
                            term.span(),
                            format!("`{}` does not apply in the `{name}` state", effect.name()),
                        );
                    }
                }
                lowered.states.push(StateStyle {
                    state: *state,
                    effects,
                });
                continue;
            }
            let effect = match name.as_str() {
                // A named corner takes its radius on every platform here;
                // the platforms where it differs get it from the rules
                // added after this one.
                "corner" if prop.value.dotted().is_some() => {
                    let name = prop.value.dotted().unwrap_or_default();
                    match sheet.corner(&name, "") {
                        Some(radius) => {
                            corner_names.push(name);
                            Some(StyleEffect::Corner { radius })
                        }
                        None => {
                            let declared = sheet
                                .corners
                                .iter()
                                .filter(|corner| corner.targets.is_empty())
                                .map(|corner| corner.name.as_str());
                            lowering.error(prop.value.span(), unknown_corner(&name, declared));
                            None
                        }
                    }
                }
                "corner" => lowering
                    .length(&prop.value, "`corner`")
                    .map(|radius| StyleEffect::Corner { radius }),
                "padding" => lowering
                    .length(&prop.value, "`padding`")
                    .map(|length| StyleEffect::Padding { length }),
                "size" => match prop.value.dotted().as_deref() {
                    Some("small") => Some(StyleEffect::Size {
                        size: ControlSize::Small,
                    }),
                    Some("regular") => Some(StyleEffect::Size {
                        size: ControlSize::Regular,
                    }),
                    Some("large") => Some(StyleEffect::Size {
                        size: ControlSize::Large,
                    }),
                    Some("extra_large") => Some(StyleEffect::Size {
                        size: ControlSize::ExtraLarge,
                    }),
                    _ => {
                        lowering.error(
                            prop.value.span(),
                            "`size` is small, regular, large or extra_large",
                        );
                        None
                    }
                },
                // `shadow: 12` at rest, or `shadow: shadow(12, y: 4)`.
                "shadow" if matches!(&prop.value, StyleValue::Call { name, .. } if name == "shadow") => {
                    lowering.effect(&prop.value, &sheet)
                }
                // `border: control.border`, or `border: border(fill.subtle, width: 0.5)`.
                "border" if matches!(&prop.value, StyleValue::Call { name, .. } if name == "border") => {
                    lowering.effect(&prop.value, &sheet)
                }
                name if Metric::ALL.iter().any(|metric| metric.name() == name) => {
                    let metric = *Metric::ALL
                        .iter()
                        .find(|metric| metric.name() == name)
                        .unwrap();
                    let fits = matches!(metric, Metric::Width | Metric::Height);
                    if !writes(metric).contains(&component.as_str()) {
                        lowering.error(
                            prop.name.span(),
                            format!(
                                "`{component}` writes no `{name}`; a style sets it on {}",
                                writes(metric).join(", ")
                            ),
                        );
                        None
                    } else if fits && prop.value.dotted().as_deref() == Some("fit") {
                        Some(StyleEffect::Metric {
                            metric,
                            value: None,
                        })
                    } else if metric == Metric::FontSize {
                        match lowering.number(&prop.value, "`font_size`") {
                            Some(size) if size > 0.0 => Some(StyleEffect::Metric {
                                metric,
                                value: Some(size),
                            }),
                            Some(_) => {
                                lowering.error(prop.value.span(), "`font_size` is more than 0");
                                None
                            }
                            None => None,
                        }
                    } else if fits && !matches!(prop.value, StyleValue::Number(..)) {
                        lowering.error(prop.value.span(), format!("`{name}` is a length or `fit`"));
                        None
                    } else {
                        lowering
                            .length(&prop.value, &format!("`{name}`"))
                            .map(|length| StyleEffect::Metric {
                                metric,
                                value: Some(length),
                            })
                    }
                }
                "background" | "foreground" | "border" | "shadow" => {
                    let call = StyleValue::Call {
                        name: prop.name.clone(),
                        args: vec![StyleArg {
                            name: None,
                            value: prop.value.clone(),
                        }],
                    };
                    lowering.effect(&call, &sheet)
                }
                other => {
                    lowering.error(
                        prop.name.span(),
                        format!(
                            "`{other}` is not a style property; use {}, a state ({}) or `settle`",
                            BASE_EFFECTS.join(", "),
                            states
                                .iter()
                                .map(|state| state.name())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    );
                    None
                }
            };
            lowered.base.extend(effect);
        }
        let platforms: Vec<String> = if lowered.targets.is_empty() {
            PLATFORMS
                .iter()
                .map(|platform| (*platform).to_string())
                .collect()
        } else {
            lowered.targets.clone()
        };
        let mut overrides = Vec::new();
        for name in &corner_names {
            let everywhere = sheet.corner(name, "");
            for platform in &platforms {
                let Some(radius) = sheet.corner(name, platform) else {
                    continue;
                };
                if Some(radius) == everywhere {
                    continue;
                }
                overrides.push(StyleRule {
                    component: lowered.component.clone(),
                    variant: lowered.variant.clone(),
                    targets: vec![platform.clone()],
                    exempt: Vec::new(),
                    base: vec![StyleEffect::Corner { radius }],
                    states: Vec::new(),
                    settle: None,
                    source: lowered.source.clone(),
                });
            }
        }
        sheet.rules.push(lowered);
        sheet.rules.extend(overrides);
    }
    sheet
}

/// Lower the `corner` blocks: the names with their radii for every
/// platform, then the values `#[target(…)]` blocks give, which may only
/// name corners declared for every platform.
fn lower_corners(lowering: &mut Lowering<'_>, item: &StyleItem, sheet: &mut StyleSheet) {
    let blocks = item
        .corners
        .iter()
        .filter(|block| block.targets.is_empty())
        .chain(
            item.corners
                .iter()
                .filter(|block| !block.targets.is_empty()),
        );
    for block in blocks {
        let mut targets = Vec::new();
        for target in &block.targets {
            let name = target.to_string();
            if PLATFORMS.contains(&name.as_str()) {
                targets.push(name);
            } else {
                lowering.error(
                    target.span(),
                    format!("`{name}` is not a platform; use {}", PLATFORMS.join(", ")),
                );
            }
        }
        if !block.targets.is_empty() && targets.is_empty() {
            continue;
        }
        let mut seen = Vec::new();
        for decl in &block.corners {
            let name = decl.name.to_string();
            if seen.contains(&name) {
                lowering.error(
                    decl.name.span(),
                    format!("corner `{name}` is named twice in this block"),
                );
                continue;
            }
            seen.push(name.clone());
            let everywhere = sheet
                .corners
                .iter()
                .any(|corner| corner.name == name && corner.targets.is_empty());
            if targets.is_empty() && everywhere {
                lowering.error(
                    decl.name.span(),
                    format!("corner `{name}` already has a radius for every platform"),
                );
                continue;
            }
            if !targets.is_empty() && !everywhere {
                lowering.error(
                    decl.name.span(),
                    format!(
                        "corner `{name}` needs a radius for every platform first, in a `corner` block without `#[target(…)]`"
                    ),
                );
                continue;
            }
            let Some(radius) = lowering.length(&decl.value, "a corner radius") else {
                continue;
            };
            sheet.corners.push(CornerDef {
                name,
                radius,
                targets: targets.clone(),
            });
        }
    }
}

/// The error for a corner name `style!` does not declare; `declared` are
/// the names it does.
pub fn unknown_corner<'a>(name: &str, declared: impl Iterator<Item = &'a str>) -> String {
    let names: Vec<&str> = declared.collect();
    if names.is_empty() {
        format!(
            "corner `{name}` is not declared; name it in a `corner` block of `style!`, such as `corner {{ {name}: 12 }}`"
        )
    } else {
        format!(
            "corner `{name}` is not declared; `style!` names {}",
            names.join(", ")
        )
    }
}

/// The variants each control kind declares, for checking `variant:`.
pub fn variants(sheet: &StyleSheet) -> Vec<(String, String)> {
    sheet
        .rules
        .iter()
        .filter_map(|rule| Some((rule.component.clone(), rule.variant.clone()?)))
        .collect()
}
