//! Controls drawn after Bern's visual specification: buttons, icon
//! buttons, switches, pickers, text fields and drop areas. A pointer press,
//! a key and an external entry (`ui.activate`, `ui.edit`, …) run the same
//! closures.

use crate::ctx::{Act, Ctx, Field, Model};
use crate::look::{self, Colors, ControlStyle, Look, Shape};
use crate::protocol::Value;
use crate::session::{Actions, Control};
use crate::units::px;
use crate::{El, icons};
use gpui::{
    App, AppContext, Entity, ExternalPaths, FontWeight, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, div,
};
use gpui_base::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use std::cell::RefCell;
use std::rc::Rc;

/// `Button.prominence`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Prominence {
    #[default]
    Standard,
    Prominent,
    Plain,
}

/// `Button.role`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    Normal,
    Destructive,
    Cancel,
}

/// How a `Button` shows and what it picks.
#[derive(Clone, Default)]
pub struct ButtonProps {
    pub label: String,
    pub icon: Option<&'static str>,
    pub prominence: Prominence,
    pub role: Role,
    pub enabled: bool,
    pub shape: Shape,
    /// A height in points in place of the size's.
    pub height: Option<f32>,
    /// The `style!` rules of its kind and variant.
    pub style: Option<&'static ControlStyle>,
    /// A draft field the open panel fills before the action runs, and the
    /// kinds of file it takes (`folder` for folders).
    pub pick: Option<(Field, &'static str)>,
    /// A key combination that presses the button, as `cmd+o`.
    pub shortcut: Option<String>,
}

/// A `MediaSource` value for a file.
fn media_source(path: &std::path::Path) -> Value {
    let url = url::Url::from_file_path(path)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.display()));
    Value::Record(vec![Value::Text(url)])
}

/// Open the system's panel for a file or a folder of `kinds`; the chosen
/// one goes into `field`, then `then` runs.
fn pick(c: &Ctx, field: Field, kinds: &'static str, then: Option<Act>, cx: &mut App) {
    let folder = kinds == "folder";
    let chosen = cx.prompt_for_paths(PathPromptOptions {
        files: !folder,
        directories: folder,
        multiple: false,
        prompt: None,
    });
    let c = c.clone();
    cx.spawn(async move |cx| {
        if let Ok(Ok(Some(paths))) = chosen.await
            && let Some(path) = paths.first()
        {
            let source = media_source(path);
            cx.update(|_| {
                if c.write(field, source)
                    && let Some(then) = &then
                {
                    then(&c);
                }
            });
        }
    })
    .detach();
}

/// The action of a pressed button: the open panel first when it picks.
pub(crate) fn press(
    c: &Ctx,
    pick_into: Option<(Field, &'static str)>,
    action: Option<Act>,
) -> crate::ctx::Press {
    let c = c.clone();
    Rc::new(move |cx: &mut App| match pick_into {
        Some((field, kinds)) => pick(&c, field, kinds, action.clone(), cx),
        None => {
            if let Some(action) = &action {
                action(&c);
            }
        }
    })
}

thread_local! {
    /// The last `clear_key` of each text field, by target.
    static CLEAR_KEYS: RefCell<std::collections::HashMap<String, i64>> = RefCell::new(std::collections::HashMap::new());
}

thread_local! {
    /// Key combinations of the buttons shown, as GPUI writes them, with
    /// what they press; rebuilt as windows draw.
    static SHORTCUTS: RefCell<Vec<(String, crate::ctx::Press)>> = const { RefCell::new(Vec::new()) };
}

/// Bern's `cmd+shift+o` as GPUI's `cmd-shift-o`.
fn keystroke(shortcut: &str) -> String {
    shortcut.replace('+', "-").to_lowercase()
}

pub(crate) fn clear_shortcuts() {
    SHORTCUTS.with(|shortcuts| shortcuts.borrow_mut().clear());
}

/// Press the button whose shortcut `keystroke` is; false when none has it.
pub(crate) fn run_shortcut(keystroke: &str, cx: &mut App) -> bool {
    let found = SHORTCUTS.with(|shortcuts| {
        shortcuts
            .borrow()
            .iter()
            .find(|(keys, _)| keys == keystroke)
            .map(|(_, press)| Rc::clone(press))
    });
    match found {
        Some(press) => {
            press(cx);
            true
        }
        None => false,
    }
}

pub(crate) fn register_shortcut(c: &Ctx, shortcut: Option<&str>, press: &crate::ctx::Press) {
    if let Some(shortcut) = shortcut
        && !c.listing()
    {
        SHORTCUTS.with(|shortcuts| {
            shortcuts
                .borrow_mut()
                .push((keystroke(shortcut), Rc::clone(press)))
        });
    }
}

thread_local! {
    /// The control under the pointer and the one being pressed, by window
    /// and target, while `style!` rules scale them.
    static POINTER: RefCell<(Option<String>, Option<String>)> = const { RefCell::new((None, None)) };
}

/// Note that the control `key` is under the pointer or pressed, or no
/// longer, and draw again.
fn track(key: &str, hovered: Option<bool>, pressed: Option<bool>, window: &mut Window) {
    POINTER.with(|pointer| {
        let mut pointer = pointer.borrow_mut();
        let set = |slot: &mut Option<String>, on: Option<bool>| match on {
            Some(true) => *slot = Some(key.to_string()),
            Some(false) if slot.as_deref() == Some(key) => *slot = None,
            _ => {}
        };
        set(&mut pointer.0, hovered);
        set(&mut pointer.1, pressed);
    });
    window.refresh();
}

/// A pressable control painted from its recipe and `style!` rules: its
/// look at rest, and under the pointer and pressed while enabled.
#[allow(clippy::too_many_arguments)]
fn pressable(
    c: &Ctx,
    id: &str,
    build: impl Fn() -> gpui::Div,
    recipe: &'static str,
    bordered: bool,
    style: Option<&'static ControlStyle>,
    enabled: bool,
    press: crate::ctx::Press,
) -> gpui::Stateful<gpui::Div> {
    let state = if enabled { "rest" } else { "disabled" };
    let rest = Colors::recipe(c, recipe, state, bordered).with(c, style.map(|style| &style.base));
    if !enabled {
        let rest = rest.with(c, style.and_then(|style| style.disabled.as_ref()));
        return look::paint(build().id(c.element_id(id)), rest);
    }
    let state_look = |name: &str, rules: Option<&Look>| -> Option<Colors> {
        match rules {
            Some(rules) if rules.none => None,
            rules => Some(
                Colors::recipe(c, recipe, name, bordered)
                    .with(c, style.map(|style| &style.base))
                    .with(c, rules),
            ),
        }
    };
    let hover = state_look("hover", style.and_then(|style| style.hover.as_ref()));
    let pressed = state_look("pressed", style.and_then(|style| style.pressed.as_ref()));
    painted(
        c,
        id,
        build,
        States {
            rest,
            hover,
            pressed,
        },
        style,
        press,
    )
}

/// A control's colors at rest, under the pointer and pressed; `None`
/// where a state gives no feedback.
pub(crate) struct States {
    pub rest: Colors,
    pub hover: Option<Colors>,
    pub pressed: Option<Colors>,
}

/// An enabled control in its colors in each state, pressed with `press`.
/// `build` makes the control at the current scale; a `style!` rule's
/// `scale` draws it again at that scale over an invisible copy that keeps
/// its place.
pub(crate) fn painted(
    c: &Ctx,
    id: &str,
    build: impl Fn() -> gpui::Div,
    states: States,
    style: Option<&'static ControlStyle>,
    press: crate::ctx::Press,
) -> gpui::Stateful<gpui::Div> {
    let States {
        rest,
        hover,
        pressed,
    } = states;
    let hover_scale = style.and_then(|style| style.hover?.scale);
    let pressed_scale = style.and_then(|style| style.pressed?.scale);
    let key = format!("{}/{}", c.win().id, c.target(id));
    let (hovering, pressing) = POINTER.with(|pointer| {
        let pointer = pointer.borrow();
        (
            pointer.0.as_deref() == Some(&key),
            pointer.1.as_deref() == Some(&key),
        )
    });
    let scale = match (pressing, hovering) {
        (true, _) => pressed_scale.or(hover_scale.filter(|_| hovering)),
        (false, true) => hover_scale,
        _ => None,
    };
    let element = match scale {
        Some((x, y)) => {
            let colors = if pressing {
                pressed.unwrap_or(rest)
            } else {
                hover.unwrap_or(rest)
            };
            let face = crate::units::scaled(x.min(y), || look::paint(build(), colors)).size_full();
            let (x, y) = ((1.0 - x) / 2.0, (1.0 - y) / 2.0);
            div()
                .id(c.element_id(id))
                .relative()
                .flex()
                .flex_none()
                .child(build().invisible())
                .child(
                    div()
                        .absolute()
                        .left(gpui::relative(x))
                        .right(gpui::relative(x))
                        .top(gpui::relative(y))
                        .bottom(gpui::relative(y))
                        .child(face),
                )
        }
        None => {
            let element = look::paint(build().id(c.element_id(id)), rest);
            let element = match hover {
                Some(hover) => element.hover(move |refinement| look::refine(refinement, hover)),
                None => element,
            };
            match pressed {
                Some(pressed) => {
                    element.active(move |refinement| look::refine(refinement, pressed))
                }
                None => element,
            }
        }
    };
    let element = if hover_scale.is_some() || pressed_scale.is_some() {
        let (hover_key, down_key, up_key, out_key) = (key.clone(), key.clone(), key.clone(), key);
        element
            .on_hover(move |hovered, window, _| track(&hover_key, Some(*hovered), None, window))
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, _| {
                track(&down_key, None, Some(true), window)
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_, window, _| {
                track(&up_key, None, Some(false), window)
            })
            .on_mouse_up_out(gpui::MouseButton::Left, move |_, window, _| {
                track(&out_key, Some(false), Some(false), window)
            })
    } else {
        element
    };
    element.on_click(move |_, _, cx| press(cx))
}

/// `Button` with a text label and an optional icon: 36 points high, 14
/// points across, 8 points round by default, in the colors of its recipe
/// and `style!` rules at rest, under the pointer, pressed and disabled.
pub fn button(c: &Ctx, id: &'static str, props: ButtonProps, action: Option<Act>) -> El {
    button_of(c, id, props, action, None)
}

/// `Button` showing `content` instead of its label, which stays its
/// accessible name.
pub fn button_with(
    c: &Ctx,
    id: &'static str,
    props: ButtonProps,
    action: Option<Act>,
    content: crate::ctx::Content<'_>,
) -> El {
    button_of(c, id, props, action, Some(content))
}

fn button_of(
    c: &Ctx,
    id: &'static str,
    props: ButtonProps,
    action: Option<Act>,
    content: Option<crate::ctx::Content<'_>>,
) -> El {
    let enabled = props.enabled && c.enabled();
    let recipe = match (props.role, props.prominence) {
        (Role::Destructive, _) => "button.destructive",
        (_, Prominence::Prominent) => "button.prominent",
        (_, Prominence::Plain) => "button.plain",
        _ => "button.standard",
    };
    let press = press(c, props.pick, action);
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "button",
        label: props.label.clone(),
        value: None,
        enabled,
        actions: Actions {
            activate: Some(Rc::clone(&press)),
            ..Actions::default()
        },
    });
    if enabled {
        register_shortcut(c, props.shortcut.as_deref(), &press);
    }
    let style = props.style;
    let (height, padding, text) = look::metrics(style.and_then(|style| style.size), props.shape);
    let height = props.height.unwrap_or(height);
    let padding = style.and_then(|style| style.padding).unwrap_or(padding);
    let corner = match props.shape {
        Shape::Capsule | Shape::Chip => height / 2.0,
        Shape::Default => style.and_then(|style| style.corner).unwrap_or(8.0),
    };
    let foreground = Colors::recipe(c, recipe, if enabled { "rest" } else { "disabled" }, false)
        .with(c, style.map(|style| &style.base))
        .foreground;
    let (icon, label) = (props.icon, props.label);
    let framed = props.shape == Shape::Capsule;
    // Content takes the button's foreground, as its label would.
    let inner = c.grown(crate::Grow::NONE).with_foreground(if framed {
        foreground
    } else {
        style
            .and_then(|style| style.base.foreground)
            .map(|foreground| crate::layout::color(c, foreground))
            .unwrap_or_else(|| c.foreground())
    });
    let group = c.reveals();
    let build = move || {
        let element = div()
            .when(group, |element| element.group(crate::layout::REVEAL))
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .rounded(px(corner))
            .text_size(px(text));
        match content {
            // A capsule is the standard button around the content.
            Some(content) if framed => element
                .px(px(padding))
                .py(px(padding.min(8.0)))
                .children(content(&inner)),
            // Otherwise the content is all there is; a style's room pads it
            // and its corner clips it.
            Some(content) => element
                .when_some(style.and_then(|style| style.padding), |element, padding| {
                    element.p(px(padding))
                })
                .when(style.and_then(|style| style.corner).is_some(), |element| {
                    element.overflow_hidden()
                })
                .children(content(&inner)),
            None => element
                .justify_center()
                .gap(px(6.0))
                .h(px(height))
                .px(px(padding))
                .font_weight(FontWeight::MEDIUM)
                .children(icon.map(|icon| icons::icon(icon, text + 2.5, foreground)))
                .child(label.clone()),
        }
    };
    let element = if content.is_some() && !framed {
        // Only `style!` rules give it a look; pressed, it darkens a little.
        let clear = Colors {
            background: c.color("transparent"),
            foreground: c.color("text.primary"),
            border: None,
            shadow: None,
            opacity: 1.0,
        };
        let rest = clear.with(c, style.map(|style| &style.base));
        if !enabled {
            look::paint(
                build().id(c.element_id(id)),
                rest.with(c, style.and_then(|style| style.disabled.as_ref())),
            )
        } else {
            let state = |rules: Option<&Look>, default: Option<Colors>| match rules {
                Some(rules) if rules.none => None,
                Some(rules) => Some(rest.with(c, Some(rules))),
                None => default,
            };
            let hover = state(style.and_then(|style| style.hover.as_ref()), None);
            let pressed = state(
                style.and_then(|style| style.pressed.as_ref()),
                Some(Colors {
                    opacity: 0.85,
                    ..rest
                }),
            );
            painted(
                c,
                id,
                build,
                States {
                    rest,
                    hover,
                    pressed,
                },
                style,
                press,
            )
        }
    } else {
        pressable(
            c,
            id,
            build,
            recipe,
            recipe == "button.standard",
            style,
            enabled,
            press,
        )
    };
    c.grow().apply(element).into_any_element()
}

/// How an `IconButton` shows and what it picks.
#[derive(Clone, Default)]
pub struct IconButtonProps {
    pub label: String,
    pub prominence: Prominence,
    pub enabled: bool,
    /// The diameter of a round button with a background.
    pub height: Option<f32>,
    /// The icon's size in points.
    pub size: Option<f32>,
    pub style: Option<&'static ControlStyle>,
    pub pick: Option<(Field, &'static str)>,
    pub shortcut: Option<String>,
}

/// `IconButton`: an icon without a container, 36 points square, its
/// accessible name `label`; with `height`, a round button of that
/// diameter.
pub fn icon_button(
    c: &Ctx,
    id: &'static str,
    icon: &'static str,
    props: IconButtonProps,
    action: Option<Act>,
) -> El {
    let enabled = props.enabled && c.enabled();
    let press = press(c, props.pick, action);
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "button",
        label: props.label,
        value: None,
        enabled,
        actions: Actions {
            activate: Some(Rc::clone(&press)),
            ..Actions::default()
        },
    });
    if enabled {
        register_shortcut(c, props.shortcut.as_deref(), &press);
    }
    let style = props.style;
    let round = props.height.is_some();
    let recipe = match (props.prominence, round) {
        (Prominence::Prominent, _) => "button.prominent",
        (Prominence::Standard, true) => "button.standard",
        _ => "icon_button",
    };
    let side = props.height.unwrap_or(36.0);
    let foreground = Colors::recipe(c, recipe, if enabled { "rest" } else { "disabled" }, false)
        .with(c, style.map(|style| &style.base))
        .foreground;
    let corner = if round {
        side / 2.0
    } else {
        style.and_then(|style| style.corner).unwrap_or(8.0)
    };
    let size = props.size.unwrap_or(18.0);
    let build = move || {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(side))
            .rounded(px(corner))
            .child(icons::icon(icon, size, foreground))
    };
    let element = pressable(
        c,
        id,
        build,
        recipe,
        recipe == "button.standard",
        style,
        enabled,
        press,
    );
    element.into_any_element()
}

/// `Switch.style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SwitchStyle {
    #[default]
    Switch,
    Checkbox,
    /// A button that stays pressed while on.
    Button,
}

/// How a `Switch` shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct SwitchProps {
    pub style: SwitchStyle,
    /// The switch alone, its label the accessible name.
    pub label_hidden: bool,
    pub icon: Option<&'static str>,
    pub shape: Shape,
    pub height: Option<f32>,
}

/// `Switch`: its label at the leading edge and the switch at the trailing
/// one, or a box before the label.
pub fn switch(c: &Ctx, id: &'static str, label: String, model: Model, props: SwitchProps) -> El {
    let style = props.style;
    let enabled = c.enabled();
    let on = model.value(c) == Value::Bool(true);
    let toggle: Rc<dyn Fn(bool)> = {
        let c = c.clone();
        let model = model.clone();
        Rc::new(move |value| model.set(&c, Value::Bool(value)))
    };
    let flip = Rc::clone(&toggle);
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: match style {
            SwitchStyle::Checkbox => "checkbox",
            SwitchStyle::Button => "toggle_button",
            SwitchStyle::Switch => "switch",
        },
        label: label.clone(),
        value: Some(on.to_string()),
        enabled,
        actions: Actions {
            activate: Some(Rc::new(move |_| flip(!on))),
            set_bool: Some(toggle.clone()),
            ..Actions::default()
        },
    });
    if style == SwitchStyle::Button {
        let (height, padding, text) = look::metrics(None, props.shape);
        let height = props.height.unwrap_or(height);
        let corner = match props.shape {
            Shape::Default => 8.0,
            _ => height / 2.0,
        };
        let colors = Colors::recipe(
            c,
            "button.standard",
            if enabled { "rest" } else { "disabled" },
            true,
        );
        let colors = if on {
            Colors {
                background: c.color("pressed.overlay"),
                foreground: c.color("accent.text"),
                ..colors
            }
        } else {
            colors
        };
        let foreground = colors.foreground;
        let element = div()
            .id(c.element_id(id))
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .h(px(height))
            .px(px(padding))
            .rounded(px(corner))
            .text_size(px(text))
            .font_weight(FontWeight::MEDIUM)
            .children(
                props
                    .icon
                    .map(|icon| icons::icon(icon, text + 2.5, foreground)),
            )
            .when(!props.label_hidden || props.icon.is_none(), |element| {
                element.child(label)
            });
        let element = look::paint(element, colors);
        if !enabled {
            return element.into_any_element();
        }
        let hover = c.color("hover.overlay");
        return element
            .hover(move |style| style.bg(hover))
            .on_click(move |_, _, _| toggle(!on))
            .into_any_element();
    }
    let state = match (enabled, on) {
        (false, _) => "disabled",
        (true, true) => "on",
        (true, false) => "off",
    };
    let text = div()
        .text_size(px(14.0))
        .line_height(px(20.0))
        .text_color(if enabled {
            c.color("text.primary")
        } else {
            c.color("disabled.foreground")
        })
        .child(label);
    let element = div()
        .id(c.element_id(id))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .min_h(px(28.0));
    let element = match style {
        SwitchStyle::Switch => {
            let track = div()
                .flex()
                .flex_none()
                .items_center()
                .w(px(36.0))
                .h(px(20.0))
                .p(px(2.0))
                .rounded_full()
                .border_1()
                .border_color(c.slot("switch", state, "border"))
                .bg(c.slot("switch", state, "track"))
                .map(|track| {
                    if on {
                        track.justify_end()
                    } else {
                        track.justify_start()
                    }
                })
                .child(
                    div()
                        .size(px(14.0))
                        .rounded_full()
                        .bg(c.slot("switch", state, "thumb")),
                );
            if props.label_hidden {
                element.child(track)
            } else {
                c.grow().apply(element).child(text.flex_1()).child(track)
            }
        }
        SwitchStyle::Checkbox => {
            let accent = c.color("accent");
            let mark = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(16.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(if on {
                    accent
                } else {
                    c.color("control.border")
                })
                .bg(if on {
                    accent
                } else {
                    c.color("surface.raised")
                })
                .children(on.then(|| icons::icon("checkmark", 12.0, c.color("on_accent"))));
            element
                .child(mark)
                .when(!props.label_hidden, |element| element.child(text))
        }
        SwitchStyle::Button => unreachable!("drawn above"),
    };
    if !enabled {
        return element.into_any_element();
    }
    element
        .on_click(move |_, _, _| toggle(!on))
        .into_any_element()
}

/// `Picker.style`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PickerStyle {
    /// A button showing the chosen option, which opens the list of them.
    #[default]
    Menu,
    /// All options side by side, the chosen one raised.
    Segmented,
}

/// `Picker`: one of `options`, each a value and its label.
#[allow(clippy::too_many_arguments)]
pub fn picker(
    c: &Ctx,
    id: &'static str,
    label: String,
    label_hidden: bool,
    model: Model,
    options: Vec<(Value, String)>,
    style: PickerStyle,
    shape: Shape,
    height: Option<f32>,
) -> El {
    let enabled = c.enabled();
    let capsule = shape != Shape::Default;
    let current = model.value(c);
    let chosen_label = options
        .iter()
        .find(|(value, _)| *value == current)
        .map(|(_, label)| label.clone())
        .unwrap_or_default();
    let choose: Rc<dyn Fn(Value)> = {
        let c = c.clone();
        let model = model.clone();
        Rc::new(move |value| model.set(&c, value))
    };
    let select: Rc<dyn Fn(&str) -> bool> = {
        let choose = Rc::clone(&choose);
        let options = options.clone();
        Rc::new(
            move |wanted: &str| match options.iter().find(|(_, label)| label == wanted) {
                Some((value, _)) => {
                    choose(value.clone());
                    true
                }
                None => false,
            },
        )
    };
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: "picker",
        label: label.clone(),
        value: Some(chosen_label.clone()),
        enabled,
        actions: Actions {
            select: Some(select),
            ..Actions::default()
        },
    });
    let field = match style {
        PickerStyle::Segmented => {
            let segments = options
                .into_iter()
                .enumerate()
                .map(|(index, (value, text))| {
                    let selected = value == current;
                    let state = if selected { "selected" } else { "rest" };
                    let segment = div()
                        .id(c.element_id(&format!("{id}.{index}")))
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(height.map(|height| height - 6.0).unwrap_or(28.0)))
                        .px(px(10.0))
                        .rounded(px(if capsule { 999.0 } else { 6.0 }))
                        .text_size(px(13.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(c.slot("segmented", state, "foreground"))
                        .bg(c.slot("segmented", state, "background"))
                        .child(text);
                    let segment = if selected {
                        segment
                            .border_1()
                            .border_color(c.slot("segmented", "selected", "border"))
                    } else {
                        segment
                    };
                    if !enabled || selected {
                        return segment;
                    }
                    let hover = c.slot("segmented", "hover", "background");
                    let choose = Rc::clone(&choose);
                    segment
                        .hover(move |style| style.bg(hover))
                        .on_click(move |_, _, _| choose(value.clone()))
                });
            c.grow()
                .apply(
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(2.0))
                        .p(px(2.0))
                        .rounded(px(if capsule { 999.0 } else { 8.0 }))
                        .border_1()
                        .border_color(c.slot("segmented", "rest", "border"))
                        .bg(c.slot("segmented", "rest", "track")),
                )
                .children(segments)
                .into_any_element()
        }
        PickerStyle::Menu => crate::overlay::menu_button(
            c,
            id,
            chosen_label,
            options
                .into_iter()
                .map(|(value, text)| {
                    let checked = value == current;
                    let choose = Rc::clone(&choose);
                    (
                        text,
                        checked,
                        Rc::new(move || choose(value.clone())) as Rc<dyn Fn()>,
                    )
                })
                .collect(),
        ),
    };
    if label_hidden {
        return field;
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.0))
        .min_h(px(36.0))
        .child(
            div()
                .flex_1()
                .text_size(px(14.0))
                .text_color(c.color("text.primary"))
                .child(label),
        )
        .child(field)
        .into_any_element()
}

/// `LanguagePicker`: the supported languages by their own names; choosing
/// one switches the interface language.
pub fn language_picker(c: &Ctx, id: &'static str, label: String) -> El {
    let current = crate::text::language();
    let options: Vec<(Value, String)> = crate::text::supported()
        .into_iter()
        .map(|tag| (Value::Text(tag.to_string()), crate::text::autonym(tag)))
        .collect();
    let model = Model::Controlled(
        Value::Text(current),
        Rc::new(|_, value| {
            if let Value::Text(tag) = value {
                crate::text::set_language(&tag);
            }
        }),
    );
    picker(
        c,
        id,
        label,
        false,
        model,
        options,
        PickerStyle::Menu,
        Shape::Default,
        None,
    )
}

/// `TextInput.kind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputKind {
    #[default]
    Text,
    Secure,
    Multiline,
    /// A search field: a magnifying glass before the text, a way to clear
    /// it after.
    Search,
}

/// How a `TextInput` shows.
#[derive(Clone, Default)]
pub struct InputProps {
    pub label: String,
    pub label_hidden: bool,
    pub prompt: String,
    pub kind: InputKind,
    /// Empty the draft after submitting (`submit: clear`).
    pub clear_on_submit: bool,
    /// A number whose change empties the field (`clear_key`), as after a
    /// message the server took.
    pub clear_key: Option<i64>,
    pub height: Option<f32>,
    pub font_size: Option<f32>,
    /// Without its own border and background, inside a panel that has them.
    pub bare: bool,
}

thread_local! {
    /// Fields with the keyboard, by key, with a count of focus changes so
    /// a late blur does not undo a newer focus.
    static FOCUS: RefCell<std::collections::HashMap<SharedString, (bool, u64)>> =
        RefCell::new(std::collections::HashMap::new());
    /// The field to take the keyboard when it next draws.
    static FOCUS_REQUEST: RefCell<Option<SharedString>> = const { RefCell::new(None) };
}

/// Whether the field `key` has the keyboard, or lost it a moment ago.
pub(crate) fn focused(key: &str) -> bool {
    FOCUS.with(|focus| focus.borrow().get(key).is_some_and(|(focused, _)| *focused))
}

/// Give the field `key` the keyboard when it next draws.
pub(crate) fn request_focus(key: &str) {
    FOCUS_REQUEST.with(|request| *request.borrow_mut() = Some(key.to_string().into()));
    crate::host::redraw();
}

fn focus_changed(key: &SharedString, focused: bool) -> u64 {
    FOCUS.with(|focus| {
        let mut focus = focus.borrow_mut();
        let entry = focus.entry(key.clone()).or_insert((false, 0));
        entry.1 += 1;
        if focused {
            entry.0 = true;
        }
        entry.1
    })
}

fn blurred(key: &SharedString, count: u64) {
    let changed = FOCUS.with(|focus| match focus.borrow_mut().get_mut(key) {
        Some(entry) if entry.1 == count && entry.0 => {
            entry.0 = false;
            true
        }
        _ => false,
    });
    if changed {
        crate::host::redraw();
    }
}

/// What a field's editing calls: the closures of its last build.
#[derive(Default)]
struct Hooks {
    change: Option<Rc<dyn Fn(String)>>,
    submit: Option<Rc<dyn Fn()>>,
}

enum Editor {
    Line(Entity<InputState>),
    Lines(Entity<TextareaState>),
}

/// A field's editing state, kept by GPUI for the element across builds.
struct FieldState {
    editor: Editor,
    hooks: Rc<RefCell<Hooks>>,
    _events: Subscription,
}

/// The field as an element: it finds or makes its editing state when it
/// is laid out, where GPUI gives it the window.
#[derive(IntoElement)]
struct TextField {
    key: SharedString,
    props: InputProps,
    value: String,
    enabled: bool,
    hooks: Hooks,
    colors: [gpui::Rgba; 3],
}

/// Run a field's hooks for `event`; focus changes are noted by `key`, a
/// blur a moment later so a press on what the field shows still lands.
fn hooked<T: 'static>(
    key: &SharedString,
    hooks: &Rc<RefCell<Hooks>>,
    event: &InputEvent,
    value: impl FnOnce() -> String,
    cx: &mut gpui::Context<T>,
) {
    match event {
        InputEvent::Focus => {
            focus_changed(key, true);
            crate::host::redraw();
        }
        InputEvent::Blur => {
            let count = focus_changed(key, false);
            let key = key.clone();
            cx.spawn(async move |_, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(180))
                    .await;
                cx.update(|_| blurred(&key, count));
            })
            .detach();
        }
        InputEvent::Change => {
            let change = hooks.borrow().change.clone();
            if let Some(change) = change {
                change(value());
            }
        }
        InputEvent::PressEnter { .. } => {
            let submit = hooks.borrow().submit.clone();
            if let Some(submit) = submit {
                submit();
            }
        }
    }
}

impl RenderOnce for TextField {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let props = self.props;
        let key = self.key.clone();
        let state = window.use_keyed_state(self.key.clone(), cx, |window, cx| {
            let hooks = Rc::new(RefCell::new(Hooks::default()));
            let events = Rc::clone(&hooks);
            match props.kind {
                InputKind::Multiline => {
                    let editor = cx
                        .new(|cx| TextareaState::new(window, cx).placeholder(props.prompt.clone()));
                    let key = key.clone();
                    let events =
                        cx.subscribe_in(&editor, window, move |_, editor, event, _, cx| {
                            let value = editor.read(cx).value().to_string();
                            hooked(&key, &events, event, || value, cx)
                        });
                    FieldState {
                        editor: Editor::Lines(editor),
                        hooks,
                        _events: events,
                    }
                }
                kind => {
                    let editor = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(props.prompt.clone())
                            .masked(kind == InputKind::Secure)
                    });
                    let key = key.clone();
                    let events =
                        cx.subscribe_in(&editor, window, move |_, editor, event, _, cx| {
                            let value = editor.read(cx).value().to_string();
                            hooked(&key, &events, event, || value, cx)
                        });
                    FieldState {
                        editor: Editor::Line(editor),
                        hooks,
                        _events: events,
                    }
                }
            }
        });
        let clear = self.hooks.change.clone();
        let has_text = !self.value.is_empty();
        *state.read(cx).hooks.borrow_mut() = self.hooks;
        let requested = FOCUS_REQUEST.with(|request| {
            let mut request = request.borrow_mut();
            let mine = request.as_ref() == Some(&self.key);
            if mine {
                *request = None;
            }
            mine
        });
        // The draft changed elsewhere (cleared after sending, an external
        // entry's edit): the field shows it. Its own edits reach the draft
        // as they happen, so a draft unlike the field changed elsewhere,
        // even while the field has the keyboard.
        let value = self.value;
        let editor = match &state.read(cx).editor {
            Editor::Line(editor) => {
                let editor = editor.clone();
                let shown = editor.read(cx).value().to_string();
                if shown != value {
                    editor.update(cx, |input, cx| input.set_value(value, window, cx));
                }
                if requested {
                    editor.update(cx, |input, cx| input.focus(window, cx));
                }
                Input::new(&editor).into_any_element()
            }
            Editor::Lines(editor) => {
                let editor = editor.clone();
                let shown = editor.read(cx).value().to_string();
                if shown != value {
                    editor.update(cx, |input, cx| input.set_value(value, window, cx));
                }
                if requested {
                    editor.update(cx, |input, cx| input.focus(window, cx));
                }
                Textarea::new(&editor).into_any_element()
            }
        };
        let [background, foreground, border] = self.colors;
        let multiline = props.kind == InputKind::Multiline;
        let search = props.kind == InputKind::Search;
        let height = props.height.unwrap_or(if search { 32.0 } else { 36.0 });
        let field = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .w_full()
            .min_h(px(height))
            .when(multiline, |field| {
                field.h(px(88.0)).items_start().py(px(8.0))
            })
            .px(px(10.0))
            .rounded(px(if search { height.min(36.0) / 2.0 } else { 8.0 }))
            .when(!props.bare, |field| {
                field.border_1().border_color(border).bg(background)
            })
            .text_color(foreground)
            .text_size(px(props.font_size.unwrap_or(14.0)))
            .when(search, |field| {
                field.child(icons::icon(
                    "magnifyingglass",
                    14.0,
                    self.colors[1].opacity(0.6),
                ))
            })
            .child(div().flex_1().child(editor))
            .when(search && has_text, |field| {
                let key = self.key.clone();
                field.child(
                    div()
                        .id("clear")
                        .flex_none()
                        .child(icons::icon(
                            "xmark.circle.fill",
                            14.0,
                            foreground.opacity(0.45),
                        ))
                        .on_click(move |_, _, _| {
                            if let Some(clear) = &clear {
                                clear(String::new());
                            }
                            request_focus(&key);
                        }),
                )
            });
        let field = if self.enabled {
            field
        } else {
            field.opacity(0.6)
        };
        if props.label_hidden {
            return field.into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .w_full()
            .child(
                div()
                    .text_size(px(12.0))
                    .line_height(px(16.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(foreground)
                    .child(props.label),
            )
            .child(field)
            .into_any_element()
    }
}

/// `TextInput`: a field editing a text draft; Enter submits.
pub fn text_input(c: &Ctx, id: &str, props: InputProps, field: Field, submit: Option<Act>) -> El {
    let enabled = c.enabled();
    let value = crate::value::as_text(&c.read(field));
    let change: Rc<dyn Fn(String)> = {
        let c = c.clone();
        Rc::new(move |text| {
            if crate::value::as_text(&c.read(field)) != text {
                c.write(field, Value::Text(text));
            }
        })
    };
    let clear_on_submit = props.clear_on_submit;
    let submit: Option<Rc<dyn Fn()>> = submit.map(|submit| {
        let c = c.clone();
        Rc::new(move || {
            submit(&c);
            if clear_on_submit {
                c.write(field, Value::Text(String::new()));
            }
        }) as Rc<dyn Fn()>
    });
    // A new clear key empties the draft once; the first one only counts.
    if let Some(key) = props.clear_key
        && !c.listing()
    {
        let target = c.target(id);
        let changed = CLEAR_KEYS.with(|keys| {
            let mut keys = keys.borrow_mut();
            let previous = keys.insert(target, key);
            previous.is_some_and(|previous| previous != key)
        });
        if changed {
            c.write(field, Value::Text(String::new()));
        }
    }
    c.list(Control {
        target: c.target(id),
        window: c.win().id.clone(),
        role: if props.kind == InputKind::Search {
            "search"
        } else {
            "text_input"
        },
        label: props.label.clone(),
        value: Some(if props.kind == InputKind::Secure {
            "•".repeat(value.chars().count())
        } else {
            value.clone()
        }),
        enabled,
        actions: Actions {
            edit: Some(Rc::clone(&change)),
            submit: submit.clone(),
            ..Actions::default()
        },
    });
    let state = if enabled { "rest" } else { "disabled" };
    let colors = [
        c.slot("field", state, "background"),
        c.slot("field", state, "foreground"),
        c.slot("field", state, "border"),
    ];
    let element = TextField {
        key: c.target(id).into(),
        props,
        value,
        enabled,
        hooks: Hooks {
            change: enabled.then_some(change),
            submit: if enabled { submit } else { None },
        },
        colors,
    };
    c.grow()
        .apply(div().flex().flex_col().child(element))
        .into_any_element()
}

/// `Drop`: content that takes a file dropped on it into `field`, then runs
/// its action; it lights up while a file is over it.
pub fn drop_area(
    c: &Ctx,
    id: &'static str,
    field: Field,
    over: Option<Field>,
    action: Option<Act>,
    children: Vec<El>,
) -> El {
    let hover = c.color("hover.overlay");
    let ctx = c.clone();
    let moved = c.clone();
    let element = div()
        .id(c.element_id(id))
        .flex()
        .flex_col()
        .rounded(px(8.0))
        .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(hover))
        // `over` follows a file held above the content.
        .when_some(over, |element, over| {
            element.on_drag_move::<ExternalPaths>(move |event, _, _| {
                let inside = event.bounds.contains(&event.event.position);
                if crate::value::truthy(&moved.read(over)) != inside {
                    moved.write(over, Value::Bool(inside));
                }
            })
        })
        .on_drop(move |paths: &ExternalPaths, _, _| {
            if let Some(over) = over {
                ctx.write(over, Value::Bool(false));
            }
            if let Some(path) = paths.paths().first()
                && ctx.write(field, media_source(path))
                && let Some(action) = &action
            {
                action(&ctx);
            }
        })
        .children(children);
    c.grow().apply(element).into_any_element()
}

/// Run the button shortcut a keystroke names, if one does.
pub(crate) fn keystroke_pressed(keystroke: &gpui::Keystroke, cx: &mut App) {
    let mut keys = Vec::new();
    let modifiers = keystroke.modifiers;
    if modifiers.platform {
        keys.push("cmd");
    }
    if modifiers.control {
        keys.push("ctrl");
    }
    if modifiers.alt {
        keys.push("alt");
    }
    if modifiers.shift {
        keys.push("shift");
    }
    if keys.is_empty() && crate::search::keystroke(&keystroke.key, cx) {
        return;
    }
    keys.push(keystroke.key.as_str());
    run_shortcut(&keys.join("-"), cx);
}

use gpui::prelude::FluentBuilder;
