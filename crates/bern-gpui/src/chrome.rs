//! The window frame where the system leaves it to the application.
//!
//! macOS draws the title bar and its buttons itself, also over content
//! that runs under a transparent title bar (`chrome: unified`).
//!
//! On Windows such a window has no system title bar. Its caption buttons
//! are drawn here in the system's shape and colors and marked as the
//! system's own areas, so Windows hit-tests them: the snap layouts flyout,
//! the window menu, dragging and double clicks behave as in any window.
//!
//! On Linux a desktop that leaves decorations to applications (GNOME on
//! Wayland), and every desktop for a unified window, which asks to draw
//! its own, gets the whole frame from here: a title bar with the buttons
//! the desktop's button layout lists, and the title for a standard window;
//! rounded corners and a shadow; edges that resize the window.

use crate::Ctx;
use crate::units::px;
#[cfg(not(target_os = "macos"))]
use gpui::prelude::FluentBuilder;
use gpui::{AnyElement, Div, IntoElement, Stateful, Styled, Window, WindowButtonLayout, div};
#[cfg(not(target_os = "macos"))]
use gpui::{InteractiveElement, ParentElement, StatefulInteractiveElement};

/// What the frame takes from a window this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    /// The title bar's height: content starts below it, or runs under it.
    pub(crate) top: f32,
    /// The shadow around the content, where the edges resize (Linux).
    pub(crate) inset: f32,
    /// The corners of the content (Linux).
    pub(crate) corner: f32,
    /// The title bar is drawn here: its buttons, and the title of a
    /// standard window.
    pub(crate) drawn: bool,
    pub(crate) titled: bool,
    /// A toolbar stands in the title bar's row: the bar is the row's
    /// height, and the toolbar's free room moves the window.
    pub(crate) toolbar: bool,
}

/// macOS's title bar, which a unified window's content runs under.
#[cfg(target_os = "macos")]
const MAC_TITLE_BAR: f32 = 28.0;

/// Windows' caption buttons, in points.
#[cfg(target_os = "windows")]
const CAPTION: (f32, f32) = (46.0, 32.0);

/// The title bar of a unified window and of a standard one, the shadow
/// around a floating window and its corners, in points (Adwaita's).
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const LINUX_BAR: (f32, f32) = (36.0, 46.0);
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const SHADOW: f32 = 12.0;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const CORNER: f32 = 12.0;

/// The title bar's height when a toolbar stands in its row: the toolbar's
/// own on macOS, whose window buttons move to its middle line as AppKit
/// puts them beside a toolbar; Windows' tall caption buttons, which
/// Windows gives a title bar with controls in it; Adwaita's header bar.
#[cfg(target_os = "macos")]
pub(crate) const TOOLBAR_ROW: f32 = 52.0;
#[cfg(target_os = "windows")]
pub(crate) const TOOLBAR_ROW: f32 = 48.0;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) const TOOLBAR_ROW: f32 = 46.0;

/// The decorations a window asks the system for: Linux draws a unified
/// window's own.
pub(crate) fn decorations(unified: bool) -> Option<gpui::WindowDecorations> {
    if cfg!(any(target_os = "linux", target_os = "freebsd")) && unified {
        Some(gpui::WindowDecorations::Client)
    } else {
        None
    }
}

/// The frame of `window` as it is now: full screen, maximized or tiled
/// windows have no shadow, and full screen no title bar of its own.
/// `toolbar`: a toolbar stood in the title bar's row the last time the
/// window drew.
#[allow(unused_variables)]
pub(crate) fn frame(window: &Window, unified: bool, toolbar: bool) -> Frame {
    #[cfg(target_os = "macos")]
    {
        Frame {
            top: match (unified, toolbar) {
                (false, _) => 0.0,
                (true, true) => TOOLBAR_ROW,
                (true, false) => MAC_TITLE_BAR,
            },
            toolbar: unified && toolbar,
            ..Frame::default()
        }
    }
    #[cfg(target_os = "windows")]
    {
        if unified && !window.is_fullscreen() {
            Frame {
                top: if toolbar { TOOLBAR_ROW } else { CAPTION.1 },
                drawn: true,
                toolbar,
                ..Frame::default()
            }
        } else {
            Frame::default()
        }
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let gpui::Decorations::Client { tiling } = window.window_decorations() else {
            return Frame::default();
        };
        if window.is_fullscreen() {
            return Frame::default();
        }
        let square =
            window.is_maximized() || tiling.top || tiling.bottom || tiling.left || tiling.right;
        Frame {
            top: match (toolbar, unified) {
                (true, _) => TOOLBAR_ROW,
                (false, true) => LINUX_BAR.0,
                (false, false) => LINUX_BAR.1,
            },
            inset: if square { 0.0 } else { SHADOW },
            corner: if square { 0.0 } else { CORNER },
            drawn: true,
            titled: !unified,
            toolbar,
        }
    }
}

/// macOS: the window buttons move to the middle line of a toolbar in the
/// title bar's row, as far from the leading edge as from the top, where
/// AppKit puts them beside a toolbar; else back where AppKit put them.
#[cfg(target_os = "macos")]
pub(crate) fn place_lights(window: &Window, win: &crate::ctx::Win, frame: Frame) {
    if win.lights.get().is_none() {
        let lights = crate::native::lights(window);
        win.lights.set(lights);
        win.lights_at
            .set(lights.map(|lights| (lights.x, lights.top)));
    }
    let Some(lights) = win.lights.get() else {
        return;
    };
    let at = if frame.toolbar {
        let inset = (frame.top - lights.height) / 2.0;
        (inset, inset)
    } else {
        (lights.x, lights.top)
    };
    if win.lights_at.get() != Some(at) {
        window.set_traffic_light_position(gpui::point(gpui::px(at.0), gpui::px(at.1)));
        win.lights_at.set(Some(at));
    }
}

/// The room the window buttons take at the leading and the trailing end
/// of a toolbar in the title bar's row, with the space between them and
/// the toolbar's items; none where there are no buttons beside it or they
/// are hidden.
#[allow(unused_variables)]
pub(crate) fn caption_room(
    frame: Frame,
    hidden: bool,
    layout: Option<WindowButtonLayout>,
    window: &Window,
    win: &crate::ctx::Win,
) -> (f32, f32) {
    #[cfg(target_os = "macos")]
    {
        match win.lights.get() {
            Some(lights) if frame.toolbar => {
                let inset = (frame.top - lights.height) / 2.0;
                (inset + lights.width + 12.0, 0.0)
            }
            _ => (0.0, 0.0),
        }
    }
    #[cfg(not(target_os = "macos"))]
    if !frame.drawn || hidden {
        return (0.0, 0.0);
    }
    // The toolbar's items keep its gap from the caption buttons.
    #[cfg(target_os = "windows")]
    {
        (0.0, 3.0 * CAPTION.0 + 8.0)
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        let controls = window.window_controls();
        let layout = layout.unwrap_or_else(WindowButtonLayout::linux_default);
        let width = |buttons: &[Option<gpui::WindowButton>]| {
            let count = buttons
                .iter()
                .flatten()
                .filter(|button| match button {
                    gpui::WindowButton::Minimize => controls.minimize,
                    gpui::WindowButton::Maximize => controls.maximize,
                    gpui::WindowButton::Close => true,
                })
                .count() as f32;
            if count == 0.0 {
                0.0
            } else {
                count * 24.0 + (count - 1.0) * 12.0 + 20.0
            }
        };
        (width(&layout.left), width(&layout.right))
    }
}

/// Free room in the title bar's row, which moves the window: Windows
/// hit-tests it as the caption; on Linux pressing it starts a move,
/// double-clicking maximizes and restores, and the secondary button opens
/// the window menu. macOS's title bar moves its window itself.
pub(crate) fn drag(area: Div) -> Div {
    #[cfg(target_os = "windows")]
    {
        area.window_control_area(gpui::WindowControlArea::Drag)
    }
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    {
        use gpui::MouseButton;
        area.on_mouse_down(MouseButton::Left, |event, window, _| {
            if event.click_count == 2 {
                window.zoom_window();
            } else {
                window.start_window_move();
            }
        })
        .on_mouse_down(MouseButton::Right, |event, window, _| {
            window.show_window_menu(event.position)
        })
    }
    #[cfg(target_os = "macos")]
    {
        area
    }
}

/// The title bar drawn over the top of the content, when the frame draws
/// one; `hidden` leaves out its buttons (an immersive window at rest).
/// `layout` is the desktop's order of the buttons (Linux).
pub(crate) fn title_bar(
    c: &Ctx,
    frame: Frame,
    title: &str,
    hidden: bool,
    layout: Option<WindowButtonLayout>,
    window: &Window,
) -> Option<AnyElement> {
    if !frame.drawn {
        return None;
    }
    let bar = div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(frame.top))
        .flex()
        .flex_row()
        .items_center();
    // Over everything, also over dialogs and toasts, as the system's own
    // title bar buttons are.
    Some(
        gpui::deferred(platform_bar(c, frame, bar, title, hidden, layout, window))
            .with_priority(10)
            .into_any_element(),
    )
}

#[cfg(target_os = "macos")]
fn platform_bar(
    _: &Ctx,
    _: Frame,
    bar: Div,
    _: &str,
    _: bool,
    _: Option<WindowButtonLayout>,
    _: &Window,
) -> AnyElement {
    bar.into_any_element()
}

/// Windows: a drag area the width of the bar, then minimize, maximize or
/// restore, and close, as the system draws them: Segoe Fluent Icons
/// glyphs (Segoe MDL2 Assets before Windows 11) on 46 points by the bar's
/// height, the close button red under the pointer. Beside a toolbar in
/// the row the toolbar's free room drags instead, so its items take
/// their clicks.
#[cfg(target_os = "windows")]
fn platform_bar(
    c: &Ctx,
    frame: Frame,
    bar: Div,
    _: &str,
    hidden: bool,
    _: Option<WindowButtonLayout>,
    window: &Window,
) -> AnyElement {
    use gpui::WindowControlArea;
    let active = window.is_window_active();
    let glyph = if active {
        c.color("text.primary")
    } else {
        c.color("text.secondary")
    };
    let font = caption_font(window);
    let hover = c.color("hover.overlay");
    let pressed = c.color("pressed.overlay");
    let button = |id: &'static str, symbol: char, area: WindowControlArea| {
        div()
            .id(id)
            .w(px(CAPTION.0))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .font_family(font)
            .text_size(px(10.0))
            .text_color(glyph)
            .window_control_area(area)
            .when(area == WindowControlArea::Close, |close| {
                let red = gpui::rgb(0xc42b1c);
                close
                    .hover(move |style| style.bg(red).text_color(gpui::white()))
                    .active(move |style| {
                        style
                            .bg(gpui::Rgba { a: 0.9, ..red })
                            .text_color(gpui::white())
                    })
            })
            .when(area != WindowControlArea::Close, |other| {
                other
                    .hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(pressed))
            })
            .child(symbol.to_string())
    };
    let free = div().flex_1().h_full();
    let bar = bar.child(if frame.toolbar { free } else { drag(free) });
    if hidden {
        return bar.into_any_element();
    }
    let restore = window.is_maximized();
    bar.child(button("bern-minimize", '\u{e921}', WindowControlArea::Min))
        .child(button(
            "bern-maximize",
            if restore { '\u{e923}' } else { '\u{e922}' },
            WindowControlArea::Max,
        ))
        .child(button("bern-close", '\u{e8bb}', WindowControlArea::Close))
        .into_any_element()
}

/// The font of the caption glyphs this system has.
#[cfg(target_os = "windows")]
fn caption_font(window: &Window) -> &'static str {
    thread_local! {
        static FONT: std::cell::OnceCell<&'static str> = const { std::cell::OnceCell::new() };
    }
    FONT.with(|font| {
        *font.get_or_init(|| {
            let fluent = "Segoe Fluent Icons";
            if window
                .text_system()
                .all_font_names()
                .iter()
                .any(|name| name == fluent)
            {
                fluent
            } else {
                "Segoe MDL2 Assets"
            }
        })
    })
}

/// Linux: the bar drags the window, double-clicking it maximizes and
/// restores, the secondary button opens the window menu, unless a toolbar
/// stands in its row, whose free room does; the buttons stand where the
/// desktop's layout puts them, as Adwaita draws them: round, 24 points,
/// tinted under the pointer.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn platform_bar(
    c: &Ctx,
    frame: Frame,
    bar: Div,
    title: &str,
    hidden: bool,
    layout: Option<WindowButtonLayout>,
    window: &Window,
) -> AnyElement {
    use gpui::{MouseButton, WindowButton};
    let layout = layout.unwrap_or_else(WindowButtonLayout::linux_default);
    let controls = window.window_controls();
    let shown = |button: &WindowButton| match button {
        WindowButton::Minimize => controls.minimize,
        WindowButton::Maximize => controls.maximize,
        WindowButton::Close => true,
    };
    let glyph = if window.is_window_active() {
        c.color("text.primary")
    } else {
        c.color("text.secondary")
    };
    let rest = gpui::Rgba {
        a: 0.1,
        ..c.color("text.primary")
    };
    let hover = gpui::Rgba {
        a: 0.15,
        ..c.color("text.primary")
    };
    let pressed = gpui::Rgba {
        a: 0.3,
        ..c.color("text.primary")
    };
    let maximized = window.is_maximized();
    let side = |buttons: &[Option<WindowButton>]| {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .px(px(10.0))
            .children(
                buttons
                    .iter()
                    .flatten()
                    .filter(|button| !hidden && shown(button))
                    .map(|&button| {
                        let symbol = match button {
                            WindowButton::Minimize => Symbol::Minimize,
                            WindowButton::Maximize if maximized => Symbol::Restore,
                            WindowButton::Maximize => Symbol::Maximize,
                            WindowButton::Close => Symbol::Close,
                        };
                        div()
                            .id(match button {
                                WindowButton::Minimize => "bern-minimize",
                                WindowButton::Maximize => "bern-maximize",
                                WindowButton::Close => "bern-close",
                            })
                            .size(px(24.0))
                            .rounded_full()
                            .bg(rest)
                            .hover(move |style| style.bg(hover))
                            .active(move |style| style.bg(pressed))
                            .child(symbol_canvas(symbol, glyph))
                            // The bar under the button does not start a move.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                match button {
                                    WindowButton::Minimize => window.minimize_window(),
                                    WindowButton::Maximize => window.zoom_window(),
                                    WindowButton::Close => window.remove_window(),
                                }
                            })
                    }),
            )
    };
    let bar = if frame.toolbar { bar } else { drag(bar) };
    bar.child(side(&layout.left))
        .child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(0.0))
                .when(frame.titled, |middle| {
                    middle.child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(glyph)
                            .truncate()
                            .child(title.to_string()),
                    )
                }),
        )
        .child(side(&layout.right))
        .into_any_element()
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[derive(Clone, Copy)]
enum Symbol {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// A window button's symbol, 16 points, drawn in `color` like Adwaita's
/// symbolic icons.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn symbol_canvas(symbol: Symbol, color: gpui::Rgba) -> impl IntoElement {
    gpui::canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let unit = f32::from(bounds.size.width) / 16.0;
            let at = |x: f32, y: f32| {
                gpui::point(
                    bounds.origin.x + gpui::px(x * unit),
                    bounds.origin.y + gpui::px(y * unit),
                )
            };
            let mut path = gpui::PathBuilder::stroke(gpui::px(1.5 * unit));
            match symbol {
                Symbol::Minimize => {
                    path.move_to(at(4.5, 11.0));
                    path.line_to(at(11.5, 11.0));
                }
                Symbol::Maximize => {
                    path.move_to(at(4.5, 4.5));
                    path.line_to(at(11.5, 4.5));
                    path.line_to(at(11.5, 11.5));
                    path.line_to(at(4.5, 11.5));
                    path.close();
                }
                Symbol::Restore => {
                    path.move_to(at(4.5, 6.5));
                    path.line_to(at(9.5, 6.5));
                    path.line_to(at(9.5, 11.5));
                    path.line_to(at(4.5, 11.5));
                    path.close();
                    path.move_to(at(6.5, 4.5));
                    path.line_to(at(11.5, 4.5));
                    path.line_to(at(11.5, 9.5));
                }
                Symbol::Close => {
                    path.move_to(at(5.0, 5.0));
                    path.line_to(at(11.0, 11.0));
                    path.move_to(at(11.0, 5.0));
                    path.line_to(at(5.0, 11.0));
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .top(px(4.0))
    .left(px(4.0))
    .size(px(16.0))
}

/// The window's content inside its frame: on Linux, with corners, a
/// shadow and edges that resize; elsewhere the content itself.
#[allow(unused_variables)]
pub(crate) fn around(c: &Ctx, frame: Frame, content: Stateful<Div>) -> AnyElement {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    if frame.drawn {
        use gpui::MouseButton;
        let inset = frame.inset;
        let dark = c.theme().dark;
        let content =
            content
                .rounded(px(frame.corner))
                .overflow_hidden()
                .when(inset > 0.0, |content| {
                    content
                        .border_1()
                        .border_color(gpui::Rgba {
                            a: if dark { 0.1 } else { 0.14 },
                            ..if dark { gpui::white() } else { gpui::black() }.into()
                        })
                        .shadow(vec![gpui::BoxShadow {
                            color: gpui::hsla(0.0, 0.0, 0.0, if dark { 0.5 } else { 0.28 }),
                            offset: gpui::point(gpui::px(0.0), gpui::px(2.0)),
                            blur_radius: gpui::px(inset - 2.0),
                            spread_radius: gpui::px(0.0),
                            inset: false,
                        }])
                });
        return div()
            .id("bern-frame")
            .size_full()
            .p(px(inset))
            .when(inset > 0.0, |frame| {
                frame.child(edges(inset)).on_mouse_down(
                    MouseButton::Left,
                    move |event, window, _| {
                        if let Some(edge) = edge(event.position, inset, window) {
                            window.start_window_resize(edge);
                        }
                    },
                )
            })
            .child(content)
            .into_any_element();
    }
    content.into_any_element()
}

/// The edge of the window under `position`, within `inset` of it.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn edge(
    position: gpui::Point<gpui::Pixels>,
    inset: f32,
    window: &Window,
) -> Option<gpui::ResizeEdge> {
    use gpui::ResizeEdge;
    let size = window.viewport_size();
    let (x, y) = (f32::from(position.x), f32::from(position.y));
    let (width, height) = (f32::from(size.width), f32::from(size.height));
    // Corners take the inset and as much again along the edges.
    let corner = inset * 2.0;
    let left = x < inset;
    let right = x > width - inset;
    let top = y < inset;
    let bottom = y > height - inset;
    let near_left = x < corner;
    let near_right = x > width - corner;
    let near_top = y < corner;
    let near_bottom = y > height - corner;
    Some(match () {
        _ if (top && near_left) || (left && near_top) => ResizeEdge::TopLeft,
        _ if (top && near_right) || (right && near_top) => ResizeEdge::TopRight,
        _ if (bottom && near_left) || (left && near_bottom) => ResizeEdge::BottomLeft,
        _ if (bottom && near_right) || (right && near_bottom) => ResizeEdge::BottomRight,
        _ if top => ResizeEdge::Top,
        _ if bottom => ResizeEdge::Bottom,
        _ if left => ResizeEdge::Left,
        _ if right => ResizeEdge::Right,
        _ => return None,
    })
}

/// The resize pointer over the edges.
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn edges(inset: f32) -> impl IntoElement {
    use gpui::{CursorStyle, ResizeEdge};
    gpui::canvas(
        |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            let Some(edge) = edge(window.mouse_position(), inset, window) else {
                return;
            };
            let cursor = match edge {
                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            };
            window.set_cursor_style(cursor, &hitbox);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}
