//! Native windows of the `WindowSet`: GPUI opens one platform window per
//! instance on every desktop, and the session runtime learns of each,
//! including window facts (`bern_window_facts`), close confirmation
//! (`bern_window_request_close` / `bern_window_confirm_close`), modal dialog
//! blocking, presentation commands and screenshots.

use crate::ctx::Win;
use crate::host::{host, status, take};
use crate::protocol::{Value, Writer, decode};
use crate::session::Control;
use crate::units::px;
use crate::{Appearance, CloseCheck, Ctx, Failure, WindowDef, text, theme};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyWindowHandle, App, AppContext, Bounds, Context, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, KeyUpEvent, MouseButton, ParentElement, Render, StatefulInteractiveElement,
    Styled, TitlebarOptions, WeakEntity, Window, WindowAppearance, WindowBounds, WindowOptions,
    div, point, size,
};
use std::cell::RefCell;
use std::rc::Rc;

/// Pending close confirmation awaiting the user or `window.confirm_close`.
#[derive(Clone, Debug)]
pub(crate) struct PendingClose {
    pub(crate) id: String,
    pub(crate) message: String,
    pub(crate) can_save: bool,
    pub(crate) draft_revision: u64,
}

/// Outcome of `window.close` / `window.confirm_close`, matching the session
/// catalog's `CloseResult` record.
pub(crate) struct CloseResult {
    pub(crate) outcome: u32, // 0 Closed, 1 Rejected, 2 ConfirmationRequired
    pub(crate) window: String,
    pub(crate) confirmation: Option<String>,
    pub(crate) draft_revision: u64,
    pub(crate) decisions: Vec<u32>,
    pub(crate) message: Option<String>,
    pub(crate) reason: Option<String>,
}

const CLOSED: u32 = 0;
const REJECTED: u32 = 1;
const CONFIRMATION_REQUIRED: u32 = 2;

impl CloseResult {
    pub(crate) fn closed(window: &str, draft_revision: u64) -> Self {
        Self {
            outcome: CLOSED,
            window: window.to_string(),
            confirmation: None,
            draft_revision,
            decisions: Vec::new(),
            message: None,
            reason: None,
        }
    }

    pub(crate) fn rejected(window: &str, draft_revision: u64, reason: String) -> Self {
        Self {
            outcome: REJECTED,
            window: window.to_string(),
            confirmation: None,
            draft_revision,
            decisions: Vec::new(),
            message: None,
            reason: Some(reason),
        }
    }

    pub(crate) fn to_value(&self) -> Value {
        let opt = |value: &Option<String>| match value {
            Some(text) => Value::some(Value::Text(text.clone())),
            None => Value::none(),
        };
        Value::Record(vec![
            Value::Enum {
                variant: self.outcome,
                fields: Vec::new(),
            },
            Value::Text(self.window.clone()),
            opt(&self.confirmation),
            Value::Int(self.draft_revision as i64),
            Value::List(
                self.decisions
                    .iter()
                    .map(|&d| Value::Enum {
                        variant: d,
                        fields: Vec::new(),
                    })
                    .collect(),
            ),
            opt(&self.message),
            opt(&self.reason),
        ])
    }
}

/// The root view of a window instance: its content, built again on every
/// render in the colors of the window's appearance.
struct WindowView {
    def: &'static WindowDef,
    win: Rc<Win>,
}

thread_local! {
    /// The root views of the open windows.
    static VIEWS: RefCell<Vec<WeakEntity<WindowView>>> = const { RefCell::new(Vec::new()) };
    /// Open dialog window instance ids; while non-empty, non-dialog windows are blocked.
    static OPEN_DIALOGS: RefCell<Vec<Rc<str>>> = const { RefCell::new(Vec::new()) };
}

/// Whether `window_id` is currently blocked by an open modal dialog window
/// or a close confirmation dialog on itself.
pub(crate) fn is_blocked(window_id: &str) -> bool {
    OPEN_DIALOGS.with(|dialogs| {
        let dialogs = dialogs.borrow();
        if !dialogs.is_empty() && !dialogs.iter().any(|id| id.as_ref() == window_id) {
            return true;
        }
        false
    })
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
        // The palette and accent the application chooses, else its brand.
        let probe = Ctx::root(
            theme::of(dark, None),
            (None, None),
            Rc::clone(&self.win),
            "",
            None,
        );
        if let Some(palette) = def.palette {
            theme::set_palette(&palette(&probe));
        }
        let seeds = match theme::palette_accent()
            .or_else(|| def.accent.and_then(|accent| accent(&probe)))
        {
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

fn resizable(def: &WindowDef) -> bool {
    (!def.utility && !def.dialog) || def.min_width.is_some() || def.min_height.is_some()
}

/// Report platform window facts to `bern_window_facts` (`source`: 0 Command, 1 System).
fn report_facts(win: &Win, def: &WindowDef, window: &Window, source: u8, force: bool) {
    let facts = crate::native::window_facts(window, resizable(def));
    if !force && win.last_facts.get() == Some(facts) {
        return;
    }
    win.last_facts.set(Some(facts));
    // SAFETY: the handle is live while windows are open.
    unsafe {
        bern_ffi::bern_window_facts(
            host().app(),
            win.kind as u32,
            win.generation,
            source,
            facts.visibility,
            facts.size,
            facts.x,
            facts.y,
            facts.width,
            facts.height,
            u8::from(facts.focused),
            facts.actions,
        );
    }
}

/// Something every window shows changed: each builds its content again,
/// at once where it is shown and when it is next shown elsewhere (macOS
/// draws no frames for a window hidden behind others).
pub(crate) fn changed(cx: &mut App) {
    for view in views() {
        let _ = view.update(cx, |_, cx| cx.notify());
    }
}

/// Redraw only the windows that read at least one of `fields`.
pub(crate) fn changed_fields(cx: &mut App, fields: &[usize]) {
    if fields.is_empty() {
        return;
    }
    for view in views() {
        let _ = view.update(cx, |window, cx| {
            if window
                .def
                .engine_fields
                .iter()
                .any(|read| fields.contains(read))
            {
                cx.notify();
            }
        });
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

/// Find an open window handle matching `window_id`.
fn find_handle(window_id: &str, cx: &mut App) -> Result<AnyWindowHandle, Failure> {
    cx.windows()
        .into_iter()
        .find(|handle| {
            handle
                .update(cx, |root, _, cx| {
                    root.downcast::<gpui_base::Root>()
                        .ok()
                        .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())
                        .is_some_and(|view| view.read(cx).win.id.as_ref() == window_id)
                })
                .unwrap_or(false)
        })
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))
}

/// Find the default window for commands that take an optional window id
/// (active window, else primary window, else first open window).
fn default_handle(cx: &mut App) -> Result<AnyWindowHandle, Failure> {
    let primary = host().def.primary;
    let windows = cx.windows();
    if let Some(active) = windows.iter().copied().find(|handle| {
        handle
            .update(cx, |_, window, _| window.is_window_active())
            .unwrap_or(false)
    }) {
        return Ok(active);
    }
    if let Some(main) = windows.iter().copied().find(|handle| {
        handle
            .update(cx, |root, _, cx| {
                root.downcast::<gpui_base::Root>()
                    .ok()
                    .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())
                    .is_some_and(|view| view.read(cx).def.id == primary)
            })
            .unwrap_or(false)
    }) {
        return Ok(main);
    }
    windows
        .into_iter()
        .next()
        .ok_or_else(|| Failure::new("target_not_found", "no open window"))
}

fn view_info(handle: AnyWindowHandle, cx: &mut App) -> Option<(&'static WindowDef, Rc<Win>)> {
    handle
        .update(cx, |root, _, cx| {
            let view = root
                .downcast::<gpui_base::Root>()
                .ok()
                .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())?;
            let read = view.read(cx);
            Some((read.def, Rc::clone(&read.win)))
        })
        .ok()
        .flatten()
}

fn evaluate_close(def: &'static WindowDef, win: &Rc<Win>) -> Option<CloseCheck> {
    let check = def.close?;
    let ctx = Ctx::root(
        theme::of(false, host().def.brand_seed),
        (host().def.brand_seed, host().def.brand_seed),
        Rc::clone(win),
        def.id,
        None,
    );
    Some(check(&ctx))
}

fn cleanup_closed(win: &Win) {
    OPEN_DIALOGS.with(|dialogs| {
        dialogs
            .borrow_mut()
            .retain(|id| id.as_ref() != win.id.as_ref())
    });
    crate::query::end(&win.id, true);
    crate::media::end(&win.id, true);
    crate::webpage::end(&win.id, true);
    crate::skeleton::end(&win.id, true);
    crate::motion::forget(&win.id);
    // SAFETY: the handle is live while windows are open.
    unsafe { bern_ffi::bern_window_closed(host().app(), win.kind as u32, win.generation) };
}

fn request_close_inner(
    def: &'static WindowDef,
    win: &Rc<Win>,
    close_handle: Option<(AnyWindowHandle, &mut App)>,
) -> Result<CloseResult, Failure> {
    let check = evaluate_close(def, win);
    let needs = check.as_ref().is_some_and(|check| check.needs_confirmation);
    let can_save = check.as_ref().is_some_and(|check| check.save.is_some());
    // SAFETY: the handle is live while the application runs.
    let bytes = take(unsafe {
        bern_ffi::bern_window_request_close(
            host().app(),
            win.kind as u32,
            win.generation,
            u8::from(needs),
            u8::from(can_save),
        )
    });
    let mut reader = status(&bytes)?;
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_reply", error.0);
    if reader.u8().map_err(malformed)? == 0 {
        win.allow_close.set(true);
        win.pending_close.borrow_mut().take();
        if let Some((handle, cx)) = close_handle {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
        return Ok(CloseResult::closed(&win.id, 0));
    }
    let pending = PendingClose {
        id: reader.text().map_err(malformed)?,
        message: check.map(|check| check.message).unwrap_or_default(),
        can_save,
        draft_revision: reader.u64().map_err(malformed)?,
    };
    *win.pending_close.borrow_mut() = Some(pending.clone());
    crate::host::redraw();
    Ok(CloseResult {
        outcome: CONFIRMATION_REQUIRED,
        window: win.id.to_string(),
        confirmation: Some(pending.id.clone()),
        draft_revision: pending.draft_revision,
        decisions: if pending.can_save {
            vec![0, 1, 2]
        } else {
            vec![1, 2]
        },
        message: Some(pending.message),
        reason: None,
    })
}

fn confirm_close_inner(
    def: &'static WindowDef,
    win: &Rc<Win>,
    handle: AnyWindowHandle,
    confirmation: &str,
    decision: u32,
    cx: &mut App,
    completion: Box<dyn FnOnce(Result<CloseResult, Failure>)>,
) {
    let mut writer = Writer::new();
    writer.text(confirmation);
    writer.u32(decision);
    // SAFETY: the handle is live; the bytes are valid for this call.
    let bytes = take(unsafe {
        bern_ffi::bern_window_confirm_close(
            host().app(),
            win.kind as u32,
            win.generation,
            writer.bytes.as_ptr(),
            writer.bytes.len(),
        )
    });
    let revision = win
        .pending_close
        .borrow()
        .as_ref()
        .map(|pending| pending.draft_revision)
        .unwrap_or(0);
    let mut reader = match status(&bytes) {
        Ok(reader) => reader,
        Err(failure) => {
            if failure.code == "stale_confirmation"
                && win
                    .pending_close
                    .borrow()
                    .as_ref()
                    .is_some_and(|pending| pending.id == confirmation)
            {
                win.pending_close.borrow_mut().take();
                changed(cx);
            }
            completion(Err(failure));
            return;
        }
    };
    let step = reader.u8().unwrap_or(2);
    win.pending_close.borrow_mut().take();
    changed(cx);
    match step {
        0 => {
            win.allow_close.set(true);
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            completion(Ok(CloseResult::closed(&win.id, revision)));
        }
        1 => {
            let Some(save) = evaluate_close(def, win).and_then(|check| check.save) else {
                completion(Err(Failure::new("invalid_params", "no save command")));
                return;
            };
            let saved_win = Rc::clone(win);
            let ctx = Ctx::root(
                theme::of(false, host().def.brand_seed),
                (host().def.brand_seed, host().def.brand_seed),
                Rc::clone(win),
                def.id,
                None,
            );
            let window_id = win.id.to_string();
            save(
                &ctx,
                Box::new(move |result| match result {
                    Ok(()) => {
                        saved_win.allow_close.set(true);
                        crate::host::window_command_close_id(window_id.clone());
                        completion(Ok(CloseResult::closed(&saved_win.id, revision)));
                    }
                    Err(failure) => completion(Ok(CloseResult::rejected(
                        &saved_win.id,
                        revision,
                        format!("save_failed: {}: {}", failure.code, failure.message),
                    ))),
                }),
            );
        }
        _ => completion(Ok(CloseResult::rejected(
            &win.id,
            revision,
            "cancelled".into(),
        ))),
    }
}

/// Render the in-window close confirmation sheet over the window content.
fn close_dialog_overlay(
    ctx: &Ctx,
    def: &'static WindowDef,
    win: &Rc<Win>,
    pending: &PendingClose,
) -> gpui::AnyElement {
    let detail = if pending.can_save {
        text::tr("bern.close.save_detail")
    } else {
        text::tr("bern.close.discard_detail")
    };
    let decide = |decision: u32| {
        let win = Rc::clone(win);
        let confirmation = pending.id.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            let handle = window.window_handle();
            confirm_close_inner(
                def,
                &win,
                handle,
                &confirmation,
                decision,
                cx,
                Box::new(|_| {}),
            );
        }
    };
    let cancel_bg = ctx.slot("button.standard", "rest", "background");
    let cancel_fg = ctx.slot("button.standard", "rest", "foreground");
    let border = ctx.color("control.border");
    let destructive_bg = ctx.slot("button.destructive", "rest", "background");
    let destructive_fg = ctx.slot("button.destructive", "rest", "foreground");
    let primary_bg = ctx.slot("button.prominent", "rest", "background");
    let primary_fg = ctx.slot("button.prominent", "rest", "foreground");

    let mut buttons = div().flex().flex_row().justify_end().gap(px(8.0));
    buttons = buttons.child(
        div()
            .id("bern-close-cancel")
            .px(px(14.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .border_1()
            .border_color(border)
            .bg(cancel_bg)
            .text_color(cancel_fg)
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .child(text::tr("bern.close.cancel"))
            .on_click(decide(2)),
    );
    buttons = buttons.child(
        div()
            .id("bern-close-discard")
            .px(px(14.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .bg(destructive_bg)
            .text_color(destructive_fg)
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .child(if pending.can_save {
                text::tr("bern.close.dont_save")
            } else {
                text::tr("bern.close.discard")
            })
            .on_click(decide(1)),
    );
    if pending.can_save {
        buttons = buttons.child(
            div()
                .id("bern-close-save")
                .px(px(14.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(primary_bg)
                .text_color(primary_fg)
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .child(text::tr("bern.close.save"))
                .on_click(decide(0)),
        );
    }

    div()
        .id("bern-close-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(ctx.color("scrim"))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .w(px(380.0))
                .p(px(20.0))
                .flex()
                .flex_col()
                .gap(px(12.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(border)
                .bg(ctx.color("surface.raised"))
                .child(
                    div()
                        .text_size(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(ctx.color("text.primary"))
                        .child(pending.message.clone()),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(ctx.color("text.secondary"))
                        .child(detail),
                )
                .child(buttons),
        )
        .into_any_element()
}

impl Render for WindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame_t0 = crate::trace::enabled().then(crate::trace::now);
        crate::host::menus_follow_language(cx);
        let viewport = window.viewport_size();
        self.win.fullscreen.set(window.is_fullscreen());
        report_facts(&self.win, self.def, window, 1, false);
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
        crate::webpage::begin(&self.win.id);
        crate::motion::begin(&self.win.id);
        crate::skeleton::begin(&self.win.id);
        crate::host::begin_openers(&self.win.id);
        self.win.bar_toolbar_built.set(false);
        self.win.has_idle_areas.set(false);
        let content = (self.def.content)(&ctx.grown(crate::Grow::ALL));
        // A toolbar came into the title bar's row or left it: the bar
        // takes its height from the next frame on.
        let toolbar = self.win.bar_toolbar_built.get();
        if toolbar != self.win.bar_toolbar.get() {
            self.win.bar_toolbar.set(toolbar);
            window.request_animation_frame();
        }
        // While something moves, the next frame shows where it is by then.
        if crate::motion::end(&self.win.id) || crate::skeleton::end(&self.win.id, false) {
            window.request_animation_frame();
        }
        crate::query::end(&self.win.id, false);
        crate::media::end(&self.win.id, false);
        crate::webpage::end(&self.win.id, false);
        crate::host::end_openers();
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
        let close_overlay = self
            .win
            .pending_close
            .borrow()
            .as_ref()
            .map(|pending| close_dialog_overlay(&ctx, self.def, &self.win, pending));
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
                let def = self.def;
                move |event: &KeyDownEvent, window, cx| {
                    if !crate::controls::is_quiet_keystroke(&event.keystroke) {
                        woke(&win);
                    }
                    if event.keystroke.key == "escape" {
                        if crate::overlay::any_open() {
                            crate::overlay::close_all();
                        } else if def.settings || def.dialog {
                            let handle = window.window_handle();
                            let _ = request_close_inner(def, &win, Some((handle, cx)));
                        }
                    }
                }
            })
            .on_key_up(|event: &KeyUpEvent, _, cx| {
                crate::controls::key_up(&event.keystroke, cx);
            })
            .on_hover({
                let win = Rc::clone(&self.win);
                move |hovered, _, _| {
                    let was_idle = win.idle();
                    win.pointer_inside.set(*hovered);
                    if !*hovered {
                        win.hovered_idle_area.set(None);
                        if !was_idle {
                            win.last_input.set(std::time::Instant::now());
                        }
                    }
                }
            })
            .on_mouse_move({
                let win = Rc::clone(&self.win);
                move |_, _, _| {
                    win.pointer_inside.set(true);
                    woke(&win)
                }
            })
            .child(content)
            .children(title_bar)
            .children(close_overlay);
        if let Some(t0) = frame_t0 {
            crate::trace::frame(&self.win.id, t0, crate::trace::now());
        }
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
            if let Some((def, win)) = view_info(handle, cx) {
                let _ = request_close_inner(def, &win, Some((handle, cx)));
            }
        }
        (crate::WindowOp::ToggleFullscreen, Some(handle)) => {
            let _ = handle.update(cx, |_, window, _| window.toggle_fullscreen());
        }
        _ => {}
    }
}

/// Force-close a window instance after an async save succeeded.
pub(crate) fn force_close_id(window_id: &str, cx: &mut App) {
    if let Ok(handle) = find_handle(window_id, cx) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}

/// Bring the primary window forward, reopening it if closed.
pub(crate) fn reopen_primary(cx: &mut App) {
    let primary = host().def.primary;
    let _ = open_kind(primary, cx);
}

/// Open (or bring forward) the application's settings window, if declared.
pub(crate) fn open_settings(cx: &mut App) {
    let def = host().def;
    if let Some(settings) = def.windows.iter().find(|w| w.settings) {
        let _ = open_kind(settings.id, cx);
    }
}

/// Open a window kind (or bring its open instance forward), returning its
/// instance id.
pub(crate) fn open_kind(kind_id: &str, cx: &mut App) -> Result<String, Failure> {
    let host = host();
    let Some(kind) = host.def.windows.iter().position(|def| def.id == kind_id) else {
        return Err(Failure::new(
            "target_not_found",
            format!("no window kind `{kind_id}`"),
        ));
    };
    for handle in cx.windows() {
        let existing = handle
            .update(cx, |root, window, cx| {
                let view = root
                    .downcast::<gpui_base::Root>()
                    .ok()
                    .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())?;
                if view.read(cx).win.kind == kind {
                    window.activate_window();
                    Some(view.read(cx).win.id.to_string())
                } else {
                    None
                }
            })
            .ok()
            .flatten();
        if let Some(id) = existing {
            return Ok(id);
        }
    }
    open(kind, &host.def.windows[kind], cx)
}

/// Request closing an open window instance by id, running the close protocol.
pub(crate) fn request_close_instance(
    window_id: &str,
    cx: &mut App,
) -> Result<CloseResult, Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    request_close_inner(def, &win, Some((handle, cx)))
}

/// Decide a pending close confirmation for `window_id`.
pub(crate) fn confirm_close_instance(
    window_id: &str,
    confirmation: &str,
    decision: u32,
    cx: &mut App,
    completion: Box<dyn FnOnce(Result<CloseResult, Failure>)>,
) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    confirm_close_inner(def, &win, handle, confirmation, decision, cx, completion);
    Ok(())
}

/// Activate (bring forward and focus) `window_id`.
pub(crate) fn activate_instance(window_id: &str, cx: &mut App) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let _ = handle.update(cx, |_, window, _| {
        window.activate_window();
        report_facts(&win, def, window, 0, true);
    });
    Ok(())
}

/// Minimize `window_id` and report updated facts.
pub(crate) fn minimize_instance(window_id: &str, cx: &mut App) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let mut outcome = Ok(());
    let _ = handle.update(cx, |_, window, _| {
        outcome = crate::native::minimize_window(window);
        if outcome.is_ok() {
            report_facts(&win, def, window, 0, true);
        }
    });
    outcome
}

/// Restore `window_id` and report updated facts.
pub(crate) fn restore_instance(window_id: &str, cx: &mut App) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let mut outcome = Ok(());
    let _ = handle.update(cx, |_, window, _| {
        outcome = crate::native::restore_window(window);
        if outcome.is_ok() {
            report_facts(&win, def, window, 0, true);
        }
    });
    outcome
}

/// Maximize `window_id` and report updated facts.
pub(crate) fn maximize_instance(window_id: &str, cx: &mut App) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let mut outcome = Ok(());
    let _ = handle.update(cx, |_, window, _| {
        outcome = crate::native::maximize_window(window);
        if outcome.is_ok() {
            report_facts(&win, def, window, 0, true);
        }
    });
    outcome
}

/// Move `window_id` to `(x, y)` and report updated facts.
pub(crate) fn move_instance(window_id: &str, x: f64, y: f64, cx: &mut App) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let mut outcome = Ok(());
    let _ = handle.update(cx, |_, window, _| {
        outcome = crate::native::move_window(window, x, y);
        if outcome.is_ok() {
            report_facts(&win, def, window, 0, true);
        }
    });
    outcome
}

/// Resize `window_id` to `(width, height)` and report updated facts.
pub(crate) fn resize_instance(
    window_id: &str,
    width: f64,
    height: f64,
    cx: &mut App,
) -> Result<(), Failure> {
    let handle = find_handle(window_id, cx)?;
    let (def, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", format!("no open window `{window_id}`")))?;
    let mut outcome = Ok(());
    let _ = handle.update(cx, |_, window, _| {
        outcome = crate::native::resize_window(window, width, height);
        if outcome.is_ok() {
            report_facts(&win, def, window, 0, true);
        }
    });
    outcome
}

/// Capture `window_id` (or the default window) as a `Screenshot` protocol record.
pub(crate) fn screenshot_instance(window_id: Option<&str>, cx: &mut App) -> Result<Value, Failure> {
    let handle = match window_id {
        Some(id) => find_handle(id, cx)?,
        None => default_handle(cx)?,
    };
    let (_, win) = view_info(handle, cx)
        .ok_or_else(|| Failure::new("target_not_found", "window is no longer open"))?;
    let mut captured: Result<(i64, i64, f64, Vec<u8>), Failure> = Err(Failure::new(
        "capture_failed",
        "window handle could not be updated",
    ));
    let _ = handle.update(cx, |_, window, _| {
        captured = crate::native::capture_window_png(window);
    });
    let (width, height, scale, png) = captured?;
    let data = crate::files::encode_base64(&png);
    Ok(Value::Record(vec![
        Value::Text(win.id.to_string()),
        Value::Int(width),
        Value::Int(height),
        Value::Float(scale),
        Value::Text("image/png".into()),
        Value::Optional(None),
        Value::Optional(Some(Box::new(Value::Text(data)))),
    ]))
}

/// Close utility windows anchored to `overlay_id` when the overlay is hidden
/// or locked.
pub(crate) fn sync_overlay(overlay_id: &str, cx: &mut App) {
    let mut ox = 0.0;
    let mut oy = 0.0;
    let mut ow = 0.0;
    let mut oh = 0.0;
    // SAFETY: valid slice pointer & length and four valid out pointers.
    let active = unsafe {
        bern_ffi::bern_overlay_rect(
            overlay_id.as_ptr(),
            overlay_id.len(),
            &mut ox,
            &mut oy,
            &mut ow,
            &mut oh,
        ) != 0
    };
    if active {
        return;
    }
    let to_close: Vec<_> = cx
        .windows()
        .into_iter()
        .filter(|handle| {
            handle
                .update(cx, |root, _, cx| {
                    root.downcast::<gpui_base::Root>()
                        .ok()
                        .and_then(|root| root.read(cx).view().clone().downcast::<WindowView>().ok())
                        .is_some_and(|view| view.read(cx).def.overlay == Some(overlay_id))
                })
                .unwrap_or(false)
        })
        .collect();
    for handle in to_close {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
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
type Instance = (u64, Rc<str>, Vec<Option<Vec<Value>>>);

fn instance(bytes: &[u8]) -> Result<Instance, Failure> {
    let host = host();
    let malformed = |error: crate::protocol::CodecError| Failure::new("invalid_reply", error.0);
    let mut reply = status(bytes)?;
    let generation = reply.u64().map_err(malformed)?;
    let id: Rc<str> = reply.text().map_err(malformed)?.into();
    let mut drafts = vec![None; host.def.sessions.len()];
    let count = reply.u32().map_err(malformed)?;
    for _ in 0..count {
        let session = reply.u32().map_err(malformed)? as usize;
        let Some(def) = host.def.sessions.get(session) else {
            return Err(Failure::new("invalid_reply", "unknown session"));
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

fn open(kind: usize, def: &'static WindowDef, cx: &mut App) -> Result<String, Failure> {
    let _ = crate::login_item::refresh();
    let host = host();
    // SAFETY: the handle is live while the application runs.
    let bytes = take(unsafe { bern_ffi::bern_window_opened(host.app(), kind as u32) });
    let (generation, id, drafts) = instance(&bytes)?;
    let width = def.width.unwrap_or(if def.utility {
        300.0
    } else if def.dialog {
        480.0
    } else {
        800.0
    }) as f32;
    let height = def.height.unwrap_or(if def.utility || def.dialog {
        360.0
    } else {
        600.0
    }) as f32;
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
    if def.dialog {
        OPEN_DIALOGS.with(|dialogs| dialogs.borrow_mut().push(Rc::clone(&win.id)));
    }
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
        kind: if def.utility || def.dialog {
            gpui::WindowKind::PopUp
        } else {
            gpui::WindowKind::Normal
        },
        is_resizable: resizable(def),
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
    let returned = win.id.to_string();
    let close_win = Rc::clone(&win);
    let facts_win = Rc::clone(&win);
    cx.open_window(options, move |window, cx| {
        if offscreen {
            crate::native::move_offscreen(window);
        }
        crate::native::apply_window_icon(window);
        report_facts(&facts_win, def, window, 1, true);
        // The close handler evaluates declared close checks before letting
        // the window close; once approved, cleans up and notifies bern-ffi.
        window.on_window_should_close(cx, move |_, _| {
            if !close_win.allow_close.get() && def.close.is_some() {
                match request_close_inner(def, &close_win, None) {
                    Ok(result) if result.outcome == CLOSED => {}
                    _ => return false,
                }
            }
            cleanup_closed(&close_win);
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
                            (after / 4.0).clamp(0.1, 0.25),
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
            // A window coming to the front tries what the network failed,
            // refreshes reported facts, and releases any held shortcut on blur.
            let act_win = Rc::clone(&win);
            cx.observe_window_activation(window, move |_, window, _| {
                report_facts(&act_win, def, window, 1, false);
                if window.is_window_active() {
                    crate::pictures::try_again();
                } else {
                    crate::controls::cancel_hold();
                }
            })
            .detach();
            // Keep reported facts in sync when the user resizes or moves.
            let bnd_win = Rc::clone(&win);
            cx.observe_window_bounds(window, move |_, window, _| {
                report_facts(&bnd_win, def, window, 1, false);
            })
            .detach();
            WindowView { def, win }
        });
        VIEWS.with(|views| views.borrow_mut().push(view.downgrade()));
        cx.new(|cx| gpui_base::Root::new(view, window, cx))
    })
    .map_err(|error| Failure::new("window_failed", error.to_string()))?;
    Ok(returned)
}
