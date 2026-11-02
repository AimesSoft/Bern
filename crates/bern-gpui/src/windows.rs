//! Native windows of the `WindowSet`: GPUI opens one platform window per
//! instance on every desktop, and the session runtime learns of each.

use crate::ctx::Win;
use crate::host::{host, status, take};
use crate::protocol::{Value, decode};
use crate::session::Control;
use crate::units::px;
use crate::{Appearance, Ctx, WindowDef, text, theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Bounds, Context, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    Render, Styled, TitlebarOptions, WeakEntity, Window, WindowAppearance, WindowBounds,
    WindowOptions, div, point, size,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The root view of a window instance: its content, built again on every
/// render in the colors of the window's appearance.
struct WindowView {
    def: &'static WindowDef,
    win: Rc<Win>,
}

thread_local! {
    /// The root views of the open windows.
    static VIEWS: RefCell<Vec<WeakEntity<WindowView>>> = const { RefCell::new(Vec::new()) };
}

/// Whether the window draws dark: the application's choice, else the
/// system's.
fn dark(win: &Rc<Win>, system: WindowAppearance) -> bool {
    let def = host().def;
    let chosen = def.appearance.map(|appearance| {
        appearance(&Ctx::root(
            theme::of(false, None),
            (None, None),
            Rc::clone(win),
            "",
            None,
        ))
    });
    match chosen {
        Some(Appearance::Light) => false,
        Some(Appearance::Dark) => true,
        _ => matches!(
            system,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

impl WindowView {
    fn ctx(&self, system: WindowAppearance, controls: Option<Rc<RefCell<Vec<Control>>>>) -> Ctx {
        let def = host().def;
        let dark = dark(&self.win, system);
        // The accent the application chooses, else its brand.
        let probe = Ctx::root(
            theme::of(dark, None),
            (None, None),
            Rc::clone(&self.win),
            "",
            None,
        );
        let seeds = match def.accent.and_then(|accent| accent(&probe)) {
            Some((light, dark)) => (Some(light), Some(dark)),
            None => (def.brand_seed, def.brand_seed),
        };
        let seed = if dark { seeds.1 } else { seeds.0 };
        Ctx::root(
            theme::of(dark, seed),
            seeds,
            Rc::clone(&self.win),
            self.def.id,
            controls,
        )
    }
}

fn views() -> Vec<WeakEntity<WindowView>> {
    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        views.retain(|view| view.upgrade().is_some());
        views.clone()
    })
}

/// Something every window shows changed: each builds its content again,
/// at once where it is shown and when it is next shown elsewhere (macOS
/// draws no frames for a window hidden behind others).
pub(crate) fn changed(cx: &mut App) {
    for view in views() {
        let _ = view.update(cx, |_, cx| cx.notify());
    }
}

/// The controls of every open window, from its content built once more
/// for the purpose.
pub(crate) fn controls(cx: &mut App) -> Vec<Control> {
    let listed = Rc::new(RefCell::new(Vec::new()));
    for view in views() {
        let _ = view.update(cx, |view, _| {
            let ctx = view.ctx(WindowAppearance::Light, Some(Rc::clone(&listed)));
            let ctx = ctx.grown(crate::Grow::ALL);
            drop((view.def.content)(&ctx));
        });
    }
    listed.take()
}

impl Render for WindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::host::menus_follow_language(cx);
        let viewport = window.viewport_size();
        self.win.fullscreen.set(window.is_fullscreen());
        // The frame the system leaves to the window: the title bar content
        // runs under or starts below, and on Linux a shadow around it.
        let frame = crate::chrome::frame(window, self.def.unified, self.win.bar_toolbar.get());
        if frame.drawn {
            window.set_client_inset(gpui::px(frame.inset));
        }
        #[cfg(target_os = "macos")]
        crate::chrome::place_lights(window, &self.win, frame);
        let top = frame.top;
        self.win.title_bar.set(top);
        self.win.inset.set(frame.inset);
        self.win.caption.set(crate::chrome::caption_room(
            frame,
            self.win.idle() && self.win.immersive.get(),
            cx.button_layout(),
            window,
            &self.win,
        ));
        self.win
            .width
            .set(f32::from(viewport.width) - 2.0 * frame.inset);
        self.win
            .height
            .set(f32::from(viewport.height) - top - 2.0 * frame.inset);
        crate::controls::clear_shortcuts();
        let ctx = self.ctx(window.appearance(), None);
        if let Some(enabled) = self.def.idle_enabled {
            let enabled = enabled(&ctx);
            if !enabled {
                self.win.last_input.set(std::time::Instant::now());
            }
            self.win.paused.set(!enabled);
        }
        if let Some(immersive) = self.def.immersive {
            self.win.immersive.set(immersive(&ctx));
        }
        // The application's icon follows the window set's choice, and the
        // menu bar's items read and write the primary window's state.
        let def = crate::host::host().def;
        if self.def.id == def.primary {
            if let Some(icon) = def.window_icon {
                crate::native::show_icon(icon(&ctx).or(def.app_icon));
            }
            if let Some(menu_bar) = def.menu_bar {
                crate::bar::show(menu_bar(&ctx), cx);
            }
        }
        crate::query::begin(&self.win.id);
        crate::media::begin(&self.win.id);
        crate::motion::begin(&self.win.id);
        self.win.bar_toolbar_built.set(false);
        let content = (self.def.content)(&ctx.grown(crate::Grow::ALL));
        // A toolbar came into the title bar's row or left it: the bar
        // takes its height from the next frame on.
        let toolbar = self.win.bar_toolbar_built.get();
        if toolbar != self.win.bar_toolbar.get() {
            self.win.bar_toolbar.set(toolbar);
            window.request_animation_frame();
        }
        // While something moves, the next frame shows where it is by then.
        if crate::motion::end(&self.win.id) {
            window.request_animation_frame();
        }
        crate::query::end(&self.win.id, false);
        crate::media::end(&self.win.id, false);
        // Under a video the window paints nothing, so the video shows.
        let canvas = ctx.color("surface.canvas");
        let video = crate::media::has_video(&self.win.id);
        // An immersive window at rest shows no title bar buttons.
        let hidden = self.win.idle() && self.win.immersive.get();
        let title_bar = crate::chrome::title_bar(
            &ctx,
            frame,
            &text::tr(self.def.title),
            hidden,
            cx.button_layout(),
            window,
        );
        let root = div()
            .id("bern-window")
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .pt(px(top))
            .map(|root| match video {
                true => root.child(crate::media::backdrop(&self.win.id, canvas, frame.corner)),
                false => root.bg(canvas),
            })
            .text_color(ctx.color("text.primary"))
            .on_key_down({
                let win = Rc::clone(&self.win);
                move |event: &KeyDownEvent, _, _| {
                    woke(&win);
                    if event.keystroke.key == "escape" {
                        crate::overlay::close_all();
                    }
                }
            })
            .on_mouse_move({
                let win = Rc::clone(&self.win);
                move |_, _, _| woke(&win)
            })
            .child(content)
            .children(title_bar);
        crate::chrome::around(&ctx, frame, root)
    }
}

/// Input arrived: the window is awake again, and shows it if it was idle.
fn woke(win: &Rc<Win>) {
    let was_idle = win.idle();
    win.last_input.set(std::time::Instant::now());
    if was_idle {
        crate::host::redraw();
    }
}

/// `BERN_WINDOW_OFFSCREEN`, for checks on a computer someone is using:
/// windows open far outside every display, where they cover nothing and,
/// never hidden behind another window, keep drawing for window captures.
/// The application stays out of the Dock and does not take the keyboard
/// from the application in use.
fn offscreen() -> bool {
    std::env::var_os("BERN_WINDOW_OFFSCREEN").is_some()
}

/// Run a window command an action asked for: open (or bring forward) a
/// window of a kind, close it, or turn full screen on or off.
pub(crate) fn command(cx: &mut App, op: crate::WindowOp, kind_id: &str) {
    let host = host();
    let Some(kind) = host.def.windows.iter().position(|def| def.id == kind_id) else {
        return;
    };
    let handle = cx.windows().into_iter().find(|handle| {
        handle
            .update(cx, |root, _, cx| {
                root.downcast::<gpui_base::Root>()
                    .ok()
                    .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())
                    .is_some_and(|view| view.read(cx).win.kind == kind)
            })
            .unwrap_or(false)
    });
    match (op, handle) {
        (crate::WindowOp::Open, None) => {
            if let Err(failure) = open(kind, &host.def.windows[kind], cx) {
                eprintln!("bern: window `{kind_id}` did not open: {}", failure.message);
            }
        }
        (crate::WindowOp::Open, Some(handle)) => {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
        (crate::WindowOp::Close, Some(handle)) => {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        (crate::WindowOp::ToggleFullscreen, Some(handle)) => {
            let _ = handle.update(cx, |_, window, _| window.toggle_fullscreen());
        }
        _ => {}
    }
}

/// Open the windows the application starts with.
pub(crate) fn open_initial(cx: &mut App) {
    if offscreen() {
        cx.set_activation_policy(gpui::ActivationPolicy::Accessory);
    }
    let host = host();
    for (kind, def) in host.def.windows.iter().enumerate() {
        if def.open
            && let Err(failure) = open(kind, def, cx)
        {
            eprintln!(
                "bern: window `{}` did not open: {}: {}",
                def.id, failure.code, failure.message
            );
        }
    }
}

/// The instance the session runtime made: its generation, its id and the
/// drafts of the sessions it hosts, by session index.
/// A window instance: its generation, id and drafts.
type Instance = (u64, Rc<str>, Vec<Option<Vec<Value>>>);

fn instance(bytes: &[u8]) -> Result<Instance, crate::Failure> {
    let host = host();
    let malformed =
        |error: crate::protocol::CodecError| crate::Failure::new("invalid_reply", error.0);
    let mut reply = status(bytes)?;
    let generation = reply.u64().map_err(malformed)?;
    let id: Rc<str> = reply.text().map_err(malformed)?.into();
    let mut drafts = vec![None; host.def.sessions.len()];
    let count = reply.u32().map_err(malformed)?;
    for _ in 0..count {
        let session = reply.u32().map_err(malformed)? as usize;
        let Some(def) = host.def.sessions.get(session) else {
            return Err(crate::Failure::new("invalid_reply", "unknown session"));
        };
        let values = def
            .fields
            .iter()
            .map(|(_, ty, _)| decode(ty, &host.def.types, &mut reply).map_err(malformed))
            .collect::<Result<Vec<_>, _>>()?;
        drafts[session] = Some(values);
    }
    Ok((generation, id, drafts))
}

fn open(kind: usize, def: &'static WindowDef, cx: &mut App) -> Result<(), crate::Failure> {
    let host = host();
    // SAFETY: the handle is live while the application runs.
    let bytes = take(unsafe { bern_ffi::bern_window_opened(host.app(), kind as u32) });
    let (generation, id, drafts) = instance(&bytes)?;
    let width = def.width.unwrap_or(800.0) as f32;
    let height = def.height.unwrap_or(600.0) as f32;
    let mut bounds = Bounds::centered(None, size(px(width), px(height)), cx);
    let offscreen = offscreen();
    // Windows would give a window outside every display a size of its own:
    // it opens on the display and moves out.
    if offscreen && cfg!(target_os = "macos") {
        bounds.origin = point(px(-10000.0), px(-10000.0));
    }
    let win = Win::new(
        kind,
        generation,
        id,
        drafts,
        (width, height),
        def.idle_after,
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: !offscreen,
        titlebar: Some(TitlebarOptions {
            title: Some(text::tr(def.title).into()),
            appears_transparent: def.unified,
            ..Default::default()
        }),
        window_min_size: match (def.min_width, def.min_height) {
            (None, None) => None,
            (min_width, min_height) => Some(size(
                px(min_width.unwrap_or(0.0) as f32),
                px(min_height.unwrap_or(0.0) as f32),
            )),
        },
        app_id: Some(host.def.bundle_id.to_string()),
        window_decorations: crate::chrome::decorations(def.unified),
        // A frame drawn on Linux has a shadow around the window.
        window_background: if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            gpui::WindowBackgroundAppearance::Transparent
        } else {
            gpui::WindowBackgroundAppearance::Opaque
        },
        ..Default::default()
    };
    let instance = win.id.clone();
    cx.open_window(options, move |window, cx| {
        if offscreen {
            crate::native::move_offscreen(window);
        }
        // The session runtime forgets the instance when it closes, and its
        // queries end.
        window.on_window_should_close(cx, move |_, _| {
            crate::query::end(&instance, true);
            crate::media::end(&instance, true);
            crate::motion::forget(&instance);
            // SAFETY: the handle is live while windows are open.
            unsafe { bern_ffi::bern_window_closed(host.app(), kind as u32, generation) };
            true
        });
        // The window goes idle when its `idle_after` passes without input.
        if let Some(after) = def.idle_after {
            let watched = Rc::downgrade(&win);
            cx.spawn(async move |cx| {
                let mut idle = false;
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs_f64(
                            (after / 4.0).clamp(0.1, 1.0),
                        ))
                        .await;
                    let Some(win) = watched.upgrade() else { break };
                    if win.idle() != idle {
                        idle = win.idle();
                        // Immersive, the pointer hides until it moves.
                        if idle && win.immersive.get() {
                            crate::native::hide_pointer();
                        }
                        cx.update(|_| crate::host::redraw());
                    }
                }
            })
            .detach();
        }
        let view = cx.new(|cx| {
            // Colors follow the window's light or dark appearance.
            cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                .detach();
            WindowView { def, win }
        });
        VIEWS.with(|views| views.borrow_mut().push(view.downgrade()));
        cx.new(|cx| gpui_base::Root::new(view, window, cx))
    })
    .map_err(|error| crate::Failure::new("window_failed", error.to_string()))?;
    Ok(())
}
