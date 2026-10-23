//! Style overrides of `style!` on buttons and sidebar items.
//!
//! Colors, borders, corners, padding, sizes and shadows become a style
//! sheet of the application's priority, one class per style and one per
//! state, so the theme's own look stays wherever a style says nothing.
//! GTK's state flags (hover, pressed, keyboard focus, disabled, selected)
//! set the state classes; looks of several states merge in the order
//! selected, hover, pressed, focus, disabled. Colors change with a short
//! CSS transition; scale, offset and opacity spring on the control's
//! motion box with the state's motion, and settle back with `settle`.

use crate::motion::{Curve, Edge, MotionBox};
use crate::theme::{self, Color};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What one state shows.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub border: Option<(Color, f64)>,
    /// A selected item filled with the color.
    pub fill: Option<Color>,
    /// A selected item marked by a bar on one edge.
    pub indicator: Option<(Color, Edge, f64)>,
    /// Blur radius, distance down, opacity.
    pub shadow: Option<(f64, f64, f64)>,
    pub scale: Option<(f64, f64)>,
    pub offset: Option<(f64, f64)>,
    pub opacity: Option<f64>,
    /// The spring of this state's transform.
    pub motion: Option<Curve>,
    /// The theme's own look in this state.
    pub system: bool,
    /// No feedback in this state.
    pub quiet: bool,
}

impl Look {
    pub const NONE: Look = Look {
        background: None,
        foreground: None,
        border: None,
        fill: None,
        indicator: None,
        shadow: None,
        scale: None,
        offset: None,
        opacity: None,
        motion: None,
        system: false,
        quiet: false,
    };
}

/// A style of one control kind, or one variant of it.
#[derive(Debug)]
pub struct StyleDef {
    /// Its class is `bern-style-<name>`.
    pub name: &'static str,
    pub base: Look,
    pub corner: Option<f64>,
    pub padding: Option<f64>,
    /// `small`, `regular`, `large` or `extra_large`.
    pub size: Option<&'static str>,
    /// Whether the style draws the control rather than keeping its look.
    pub draws: bool,
    pub hover: Option<Look>,
    pub pressed: Option<Look>,
    pub selected: Option<Look>,
    pub focus: Option<Look>,
    pub disabled: Option<Look>,
    pub settle: Option<Curve>,
}

const STATES: [&str; 5] = ["selected", "hover", "pressed", "focus", "disabled"];

impl StyleDef {
    fn state(&self, name: &str) -> Option<&Look> {
        match name {
            "selected" => self.selected.as_ref(),
            "hover" => self.hover.as_ref(),
            "pressed" => self.pressed.as_ref(),
            "focus" => self.focus.as_ref(),
            "disabled" => self.disabled.as_ref(),
            _ => None,
        }
    }
}

/// The weight and size `text { … }` gives a text style: its name, one of
/// `ultralight` … `black`, and its size in points.
pub type TextStyle = (&'static str, &'static str, Option<f64>);

thread_local! {
    static STYLES: Cell<&'static [StyleDef]> = const { Cell::new(&[]) };
    static TEXT: Cell<&'static [TextStyle]> = const { Cell::new(&[]) };
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
    /// Rules made at run time for values the declaration gives, such as a
    /// row height: one class per distinct body.
    static RULES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The class of a rule with this body, made the first time it is asked
/// for. Only for values written in the declaration: each new one writes
/// the style sheet again.
pub(crate) fn class_for(body: &str) -> String {
    class_with(&format!("& {{ {body} }}"))
}

/// The class of rules where `&` stands for the class's selector.
pub(crate) fn class_with(rules: &str) -> String {
    let (class, added) = RULES.with(|existing| {
        let mut existing = existing.borrow_mut();
        match existing.iter().position(|kept| kept == rules) {
            Some(index) => (format!("bern-rule-{index}"), false),
            None => {
                existing.push(rules.to_string());
                (format!("bern-rule-{}", existing.len() - 1), true)
            }
        }
    });
    if added {
        reload();
    }
    class
}

/// The CSS weight of a text style's weight.
fn weight(name: &str) -> Option<u16> {
    Some(match name {
        "ultralight" => 100,
        "thin" => 200,
        "light" => 300,
        "regular" => 400,
        "medium" => 500,
        "semibold" => 600,
        "bold" => 700,
        "heavy" => 800,
        "black" => 900,
        _ => return None,
    })
}

/// The class a `Text` or `Label` of `style` carries for the weights of
/// `text { … }`.
pub(crate) fn text_class(style: &str) -> String {
    format!("bern-text-{style}")
}

fn shadows(look: &Look, part: Option<bool>) -> Option<String> {
    let mut shadows = Vec::new();
    if let Some((radius, y, opacity)) = look.shadow {
        shadows.push(format!("0 {y}px {radius}px rgba(0, 0, 0, {opacity})"));
    }
    if let Some((color, edge, width)) = &look.indicator {
        let offset = match edge {
            Edge::Leading => format!("{width}px 0"),
            Edge::Trailing => format!("-{width}px 0"),
            Edge::Top => format!("0 {width}px"),
            Edge::Bottom => format!("0 -{width}px"),
        };
        shadows.push(format!("inset {offset} 0 0 {}", theme::css(color, part)));
    }
    (!shadows.is_empty()).then(|| shadows.join(", "))
}

/// The declarations of one look; `base` is the style's resting look, for
/// states that show no feedback.
fn declarations(look: &Look, base: &Look, part: Option<bool>) -> String {
    let mut out = String::new();
    if look.system {
        return out;
    }
    if look.quiet {
        let background = base
            .background
            .map(|color| theme::css(&color, part))
            .unwrap_or_else(|| "transparent".into());
        out.push_str(&format!(
            "background-color: {background}; background-image: none; box-shadow: none; "
        ));
        return out;
    }
    let fill = look.fill.or(look.background);
    if let Some(color) = fill {
        out.push_str(&format!(
            "background-color: {}; background-image: none; ",
            theme::css(&color, part)
        ));
    }
    let foreground = look
        .foreground
        .or_else(|| fill.and_then(|fill| theme::pair(&fill)));
    if let Some(color) = foreground {
        out.push_str(&format!("color: {}; ", theme::css(&color, part)));
    }
    if let Some((color, width)) = &look.border {
        out.push_str(&format!(
            "border: {width}px solid {}; ",
            theme::css(color, part)
        ));
    }
    if let Some(shadows) = shadows(look, part) {
        out.push_str(&format!("box-shadow: {shadows}; "));
    }
    out
}

fn size_declarations(size: &str) -> &'static str {
    match size {
        "small" => "min-height: 24px; padding: 2px 10px; font-size: smaller; ",
        "large" => "min-height: 44px; padding: 10px 22px; font-size: larger; ",
        "extra_large" => "min-height: 52px; padding: 14px 28px; font-size: x-large; ",
        _ => "",
    }
}

/// The style sheet of every style, for the current appearance and for
/// parts with an appearance of their own.
fn sheet(styles: &[StyleDef], text: &[TextStyle]) -> String {
    let mut css = String::from(
        // A styled sidebar row leaves its look to the content it holds.
        ".navigation-sidebar > row.bern-styled-row { padding: 0; background: none; box-shadow: none; }\n\
         .navigation-sidebar > row.bern-styled-row:focus-visible { outline-offset: -2px; }\n\
         .navigation-sidebar > row:selected .bern-sidebar-label { font-weight: 700; }\n\
         .bern-sidebar-content { padding: 8px 10px; border-radius: 6px; min-height: 20px; }\n\
         .bern-sidebar-content.bern-hover { background-color: alpha(currentColor, 0.07); }\n\
         .bern-sidebar-content.bern-selected { background-color: alpha(currentColor, 0.1); }\n\
         .bern-sidebar-content.bern-pressed { background-color: alpha(currentColor, 0.16); }\n\
         .bern-content-button { padding: 0; min-width: 0; min-height: 0; background: none; box-shadow: none; border-radius: 0; }\n\
         .bern-content-plain:active { filter: brightness(0.85); }\n",
    );
    css.push_str(crate::scene::SHEET);
    for (style, name, size) in text {
        let Some(weight) = weight(name) else { continue };
        css.push_str(&format!(
            ".{} {{ font-weight: {weight}; ",
            text_class(style)
        ));
        if let Some(size) = size {
            css.push_str(&format!("font-size: {size}px; "));
        }
        css.push_str("}\n");
    }
    RULES.with(|rules| {
        for (index, template) in rules.borrow().iter().enumerate() {
            css.push_str(&template.replace('&', &format!(".bern-rule-{index}")));
            css.push('\n');
        }
    });
    for (scope, part) in [
        ("", None),
        (".bern-appearance-dark ", Some(true)),
        (".bern-appearance-light ", Some(false)),
    ] {
        for style in styles {
            let class = format!("{scope}.bern-style-{}", style.name);
            let mut base = declarations(&style.base, &Look::NONE, part);
            if part.is_none() {
                base.push_str(
                    "transition: background-color 150ms ease-out, color 150ms ease-out, box-shadow 150ms ease-out, border-color 150ms ease-out; ",
                );
                if let Some(corner) = style.corner {
                    base.push_str(&format!("border-radius: {corner}px; "));
                }
                if let Some(size) = style.size {
                    base.push_str(size_declarations(size));
                }
                if let Some(padding) = style.padding {
                    base.push_str(&format!("padding: {padding}px; "));
                }
            }
            if !base.is_empty() {
                css.push_str(&format!("{class} {{ {base}}}\n"));
            }
            for state in STATES {
                if let Some(look) = style.state(state) {
                    let body = declarations(look, &style.base, part);
                    if !body.is_empty() {
                        css.push_str(&format!("{class}.bern-{state} {{ {body}}}\n"));
                    }
                }
            }
        }
    }
    css
}

/// Re-resolve styled descendants inside a popup that follows its window,
/// even when its trigger is nested in a part with a fixed appearance.
pub(crate) fn scoped_colors(scope: &str, dark: bool) -> String {
    let mut css = String::new();
    for style in STYLES.with(Cell::get) {
        let class = format!("{scope} .bern-style-{}", style.name);
        css.push_str(&format!("{class} {{ {} }}\n", declarations(&style.base, &Look::NONE, Some(dark))));
        for state in STATES {
            if let Some(look) = style.state(state) {
                css.push_str(&format!("{class}.bern-{state} {{ {} }}\n", declarations(look, &style.base, Some(dark))));
            }
        }
    }
    css
}

/// Install the styles of the application; called once at startup.
pub fn install(styles: &'static [StyleDef], text: &'static [TextStyle]) {
    STYLES.with(|cell| cell.set(styles));
    TEXT.with(|cell| cell.set(text));
    reload();
}

/// Write the style sheet again, as after the appearance changed.
pub(crate) fn reload() {
    let styles = STYLES.with(Cell::get);
    let css = sheet(styles, TEXT.with(Cell::get));
    PROVIDER.with(|provider| {
        let mut provider = provider.borrow_mut();
        let provider = provider.get_or_insert_with(|| {
            let provider = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            provider
        });
        provider.load_from_string(&css);
    });
}

/// Show `style` on a surface: a stack's background, foreground and
/// corners. The corners clip what the layers draw.
pub(crate) fn surface(style: &'static StyleDef, widget: &gtk::Widget) {
    widget.add_css_class(&format!("bern-style-{}", style.name));
    if style.corner.is_some() {
        widget.set_overflow(gtk::Overflow::Hidden);
    }
}

/// Show `style` on a control: `states` is the widget whose GTK state flags
/// say which states hold, `styled` the widget that draws the look, and
/// `motion` the box its transform springs on.
pub(crate) fn apply(
    style: &'static StyleDef,
    states: &gtk::Widget,
    styled: &gtk::Widget,
    motion: &MotionBox,
) {
    styled.add_css_class(&format!("bern-style-{}", style.name));
    let layer = motion.layer("state");
    let last: Rc<Cell<Option<Curve>>> = Rc::new(Cell::new(None));
    let shown: Rc<RefCell<[bool; 5]>> = Rc::new(RefCell::new([false; 5]));
    let update = {
        let styled = styled.clone();
        move |flags: gtk::StateFlags| {
            let held = [
                flags.contains(gtk::StateFlags::SELECTED)
                    || flags.contains(gtk::StateFlags::CHECKED),
                flags.contains(gtk::StateFlags::PRELIGHT),
                flags.contains(gtk::StateFlags::ACTIVE),
                flags.contains(gtk::StateFlags::FOCUS_VISIBLE),
                flags.contains(gtk::StateFlags::INSENSITIVE),
            ];
            if *shown.borrow() == held {
                return;
            }
            *shown.borrow_mut() = held;
            for (state, on) in STATES.iter().zip(held) {
                let class = format!("bern-{state}");
                if on {
                    styled.add_css_class(&class);
                } else {
                    styled.remove_css_class(&class);
                }
            }
            let looks: Vec<&Look> = std::iter::once(&style.base)
                .chain(
                    STATES
                        .iter()
                        .zip(held)
                        .filter(|(_, on)| *on)
                        .filter_map(|(state, _)| style.state(state)),
                )
                .collect();
            let mut scale = (1.0, 1.0);
            let mut offset = (0.0, 0.0);
            let mut opacity = 1.0;
            for look in &looks {
                if look.system || look.quiet {
                    continue;
                }
                if let Some(value) = look.scale {
                    scale = value;
                }
                if let Some(value) = look.offset {
                    offset = value;
                }
                if let Some(value) = look.opacity {
                    opacity = value;
                }
            }
            // The motion of the topmost state that moves; back at rest,
            // `settle`.
            let moving = looks.iter().rev().find_map(|look| look.motion);
            let curve = moving.or(style.settle).or(last.get());
            if moving.is_some() {
                last.set(moving);
            }
            let to = |spring: &crate::motion::Spring, value: f64| {
                if (spring.target() - value).abs() > f64::EPSILON {
                    spring.to(value, curve, None);
                }
            };
            to(&layer.sx, scale.0);
            to(&layer.sy, scale.1);
            to(&layer.x, offset.0);
            to(&layer.y, offset.1);
            to(&layer.opacity, opacity);
        }
    };
    let update = Rc::new(update);
    update(states.state_flags());
    let changed = Rc::clone(&update);
    let handler =
        states.connect_state_flags_changed(move |widget, _| changed(widget.state_flags()));
    let watched = states.downgrade();
    crate::reactive::on_cleanup(move || {
        if let Some(widget) = watched.upgrade() {
            widget.disconnect(handler);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: Color = Color::Role("accent");

    fn style(name: &'static str, base: Look) -> StyleDef {
        StyleDef {
            name,
            base,
            corner: None,
            padding: None,
            size: None,
            draws: true,
            hover: None,
            pressed: None,
            selected: None,
            focus: None,
            disabled: None,
            settle: None,
        }
    }

    #[test]
    fn sizes_set_height_padding_and_font() {
        assert_eq!(
            size_declarations("small"),
            "min-height: 24px; padding: 2px 10px; font-size: smaller; "
        );
        assert!(size_declarations("extra_large").starts_with("min-height: 52px; "));
        assert_eq!(size_declarations("regular"), "");
    }

    #[test]
    fn shadows_and_indicators_share_the_box_shadow() {
        let shadow = Look {
            shadow: Some((8.0, 2.0, 0.2)),
            ..Look::NONE
        };
        assert_eq!(
            shadows(&shadow, None).as_deref(),
            Some("0 2px 8px rgba(0, 0, 0, 0.2)")
        );
        let bar = |edge| Look {
            indicator: Some((ACCENT, edge, 3.0)),
            ..Look::NONE
        };
        for (edge, offset) in [
            (Edge::Leading, "3px 0"),
            (Edge::Trailing, "-3px 0"),
            (Edge::Top, "0 3px"),
            (Edge::Bottom, "0 -3px"),
        ] {
            assert_eq!(
                shadows(&bar(edge), None),
                Some(format!("inset {offset} 0 0 @accent_bg_color"))
            );
        }
        let both = Look {
            shadow: shadow.shadow,
            ..bar(Edge::Leading)
        };
        assert_eq!(
            shadows(&both, None).as_deref(),
            Some("0 2px 8px rgba(0, 0, 0, 0.2), inset 3px 0 0 0 @accent_bg_color")
        );
        assert_eq!(shadows(&Look::NONE, None), None);
    }

    #[test]
    fn looks_declare_fill_text_and_border() {
        let accent = Look {
            background: Some(ACCENT),
            border: Some((Color::Role("control.border"), 1.0)),
            ..Look::NONE
        };
        assert_eq!(
            declarations(&accent, &Look::NONE, None),
            "background-color: @accent_bg_color; background-image: none; color: @accent_fg_color; border: 1px solid alpha(@window_fg_color, 0.15); "
        );
        // A selected fill covers the background; a foreground of its own
        // wins over the fill's pair.
        let selected = Look {
            background: Some(Color::Role("surface.panel")),
            fill: Some(Color::Role("status.error")),
            foreground: Some(Color::Role("text.primary")),
            ..Look::NONE
        };
        assert_eq!(
            declarations(&selected, &Look::NONE, None),
            "background-color: @error_bg_color; background-image: none; color: @window_fg_color; "
        );
        let panel = Look {
            background: Some(Color::Role("surface.panel")),
            ..Look::NONE
        };
        assert_eq!(
            declarations(&panel, &Look::NONE, None),
            "background-color: @card_bg_color; background-image: none; "
        );
    }

    #[test]
    fn system_and_quiet_states() {
        let base = Look {
            background: Some(Color::Role("surface.panel")),
            ..Look::NONE
        };
        let system = Look {
            system: true,
            background: Some(ACCENT),
            ..Look::NONE
        };
        assert_eq!(declarations(&system, &base, None), "");
        // No feedback: the resting background, nothing drawn over it.
        let quiet = Look {
            quiet: true,
            background: Some(ACCENT),
            ..Look::NONE
        };
        assert_eq!(
            declarations(&quiet, &base, None),
            "background-color: @card_bg_color; background-image: none; box-shadow: none; "
        );
        assert_eq!(
            declarations(&quiet, &Look::NONE, None),
            "background-color: transparent; background-image: none; box-shadow: none; "
        );
    }

    #[test]
    fn the_sheet_has_a_rule_per_style_and_state() {
        let mut pill = style(
            "pill",
            Look {
                background: Some(ACCENT),
                ..Look::NONE
            },
        );
        pill.corner = Some(12.0);
        pill.size = Some("small");
        pill.padding = Some(4.0);
        pill.hover = Some(Look {
            background: Some(Color::Role("status.success")),
            ..Look::NONE
        });
        pill.pressed = Some(Look {
            system: true,
            ..Look::NONE
        });
        let css = sheet(&[pill, style("plain", Look::NONE)], &[]);
        let rule = |selector: &str| {
            css.lines()
                .find(|line| line.starts_with(&format!("{selector} {{")))
                .map(str::to_string)
        };
        let base = rule(".bern-style-pill").expect("the pill's rule");
        assert!(base.starts_with(
            ".bern-style-pill { background-color: @accent_bg_color; background-image: none; color: @accent_fg_color; transition: "
        ));
        // The declared padding comes after the size's.
        assert!(base.ends_with(
            "border-radius: 12px; min-height: 24px; padding: 2px 10px; font-size: smaller; padding: 4px; }"
        ));
        assert_eq!(
            rule(".bern-style-pill.bern-hover").as_deref(),
            Some(
                ".bern-style-pill.bern-hover { background-color: @success_bg_color; background-image: none; }"
            )
        );
        // A state with the theme's own look adds no rule.
        assert_eq!(rule(".bern-style-pill.bern-pressed"), None);
        // Parts with an appearance of their own restate only the colors.
        assert_eq!(
            rule(".bern-appearance-dark .bern-style-pill").as_deref(),
            Some(
                ".bern-appearance-dark .bern-style-pill { background-color: @accent_bg_color; background-image: none; color: @accent_fg_color; }"
            )
        );
        assert!(rule(".bern-style-plain").is_some_and(|rule| rule.contains("transition: ")));
        assert_eq!(rule(".bern-appearance-light .bern-style-plain"), None);
    }
}
