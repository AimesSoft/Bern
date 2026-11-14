//! Native pure-Rust GTK 4 presenter for `Svg` (`bern-svg` + `MemoryTexture`),
//! plus optional `WebPage` (`webkit6`, enabled only when `WebPage` is used).

use crate::reactive::{effect, on_cleanup};
use crate::{Ctx, Get, Num, Parent, Str};
use bern_ffi::bern_svg::{Config, Fit, PixelFormat, Player, PointerEvent};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub struct SvgProps {
    pub source: &'static str,
    pub label: Option<Str>,
    pub width: Option<Num>,
    pub height: Option<Num>,
    pub fit: Option<&'static str>,
    pub playing: Option<Get>,
    pub speed: Option<Get>,
    pub restart: Option<Get>,
    pub interactive: bool,
    pub data: Option<Str>,
    pub external_links: bool,
}

fn resolve_fit(fit: Option<&str>) -> Fit {
    match fit {
        Some("cover") => Fit::Cover,
        Some("fill") => Fit::Fill,
        _ => Fit::Contain,
    }
}

struct GtkSvgState {
    player: Option<Player>,
    sizes: (f64, f64),
    playing: bool,
    speed: f64,
    restart: f64,
    fit: Fit,
    interactive: bool,
    active: bool,
    tick_id: Option<gtk::TickCallbackId>,
}

impl GtkSvgState {
    fn render_into(&mut self, pic: &gtk::Picture, now_secs: f64) {
        let Some(ref mut player) = self.player else {
            self.active = false;
            return;
        };
        let mapped = pic.is_mapped();
        let dark = crate::theme::part_of(pic.upcast_ref()).unwrap_or_else(crate::theme::is_dark);
        let text_rgba = if dark {
            [255, 255, 255, 255]
        } else {
            [0, 0, 0, 255]
        };
        let _ = player.configure(Config {
            playing: self.playing && mapped,
            speed: if self.speed.is_finite() && self.speed > 0.0 {
                self.speed
            } else {
                1.0
            },
            restart: if self.restart.is_finite() {
                self.restart
            } else {
                0.0
            },
            fit: self.fit,
            interactive: self.interactive,
            dark,
            reduced_motion: crate::motion::reduced(),
            text_rgba,
        });

        let scale = pic.native().and_then(|n| n.surface()).map_or_else(
            || f64::from(pic.scale_factor().max(1)),
            |s| s.scale().max(1.0),
        );
        let w_px = ((self.sizes.0.max(1.0) * scale).round() as i32).max(1);
        let h_px = ((self.sizes.1.max(1.0) * scale).round() as i32).max(1);

        let (outcome, pixels) =
            player.render(now_secs, w_px as u32, h_px as u32, PixelFormat::RgbaPremul);
        if outcome.changed && pixels.len() == (w_px as usize) * (h_px as usize) * 4 {
            let bytes = glib::Bytes::from(pixels);
            let tex = gtk::gdk::MemoryTexture::new(
                w_px,
                h_px,
                gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
                &bytes,
                (w_px as usize) * 4,
            );
            pic.set_paintable(Some(&tex));
        }
        self.active = outcome.active;
    }
}

fn ensure_tick(state: &Rc<RefCell<GtkSvgState>>, pic: &gtk::Picture) {
    let want = {
        let s = state.borrow();
        s.player.is_some() && s.playing && s.active && pic.is_mapped() && s.tick_id.is_none()
    };
    if !want {
        return;
    }
    let st_cb = Rc::clone(state);
    let id = pic.add_tick_callback(move |pic, clock| {
        let now_secs = clock.frame_time() as f64 * 1e-6;
        let mut s = st_cb.borrow_mut();
        s.render_into(pic, now_secs);
        if s.playing && s.active && pic.is_mapped() {
            glib::ControlFlow::Continue
        } else {
            s.tick_id = None;
            glib::ControlFlow::Break
        }
    });
    state.borrow_mut().tick_id = Some(id);
}

fn monotonic_secs() -> f64 {
    glib::monotonic_time() as f64 * 1e-6
}

#[allow(non_snake_case)]
pub fn Svg(p: &Parent, ctx: &Ctx, props: SvgProps) {
    let pic = gtk::Picture::new();
    pic.set_content_fit(gtk::ContentFit::Fill);
    pic.set_can_shrink(true);

    let source_path = std::path::Path::new(&crate::host::resources_dir()).join(props.source);
    let player = match std::fs::read(&source_path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| bern_ffi::svg::open_scene(props.source, &bytes))
    {
        Ok(scene) => Some(Player::new(scene)),
        Err(error) => {
            eprintln!("Bern SVG {}: {error}", source_path.display());
            None
        }
    };

    let state = Rc::new(RefCell::new(GtkSvgState {
        player,
        sizes: (0.0, 0.0),
        playing: true,
        speed: 1.0,
        restart: 0.0,
        fit: resolve_fit(props.fit),
        interactive: props.interactive,
        active: true,
        tick_id: None,
    }));

    let frame = crate::picture::Frame::new(&pic, 0, 0);
    let cell_sizes = Rc::new(Cell::new((0, 0)));
    if let Some(width) = &props.width {
        let cell_sizes = cell_sizes.clone();
        let frame = frame.clone();
        let st = Rc::clone(&state);
        let pic_ref = pic.clone();
        crate::motion::bind_number(ctx, width, None, &pic, move |n| {
            let w = n.max(0.0);
            cell_sizes.set((w.round() as i32, cell_sizes.get().1));
            frame.set_size(cell_sizes.get().0, cell_sizes.get().1);
            {
                let mut s = st.borrow_mut();
                s.sizes.0 = w;
                s.render_into(&pic_ref, monotonic_secs());
            }
            ensure_tick(&st, &pic_ref);
        });
    }
    if let Some(height) = &props.height {
        let cell_sizes = cell_sizes.clone();
        let frame = frame.clone();
        let st = Rc::clone(&state);
        let pic_ref = pic.clone();
        crate::motion::bind_number(ctx, height, None, &pic, move |n| {
            let h = n.max(0.0);
            cell_sizes.set((cell_sizes.get().0, h.round() as i32));
            frame.set_size(cell_sizes.get().0, cell_sizes.get().1);
            {
                let mut s = st.borrow_mut();
                s.sizes.1 = h;
                s.render_into(&pic_ref, monotonic_secs());
            }
            ensure_tick(&st, &pic_ref);
        });
    }

    if let Some(label) = props.label {
        let ctx = ctx.clone();
        let pic_ref = pic.clone();
        effect(move || pic_ref.update_property(&[gtk::accessible::Property::Label(&label(&ctx))]));
    } else {
        pic.update_state(&[gtk::accessible::State::Hidden(true)]);
    }

    {
        let ctx = ctx.clone();
        let st = Rc::clone(&state);
        let pic_ref = pic.clone();
        let playing_prop = props.playing;
        let speed_prop = props.speed;
        let restart_prop = props.restart;
        effect(move || {
            crate::theme::track();
            let number = |get: &Option<Get>, fallback| {
                get.as_ref()
                    .map(|get| crate::value::as_f64(&get(&ctx)))
                    .filter(|n| n.is_finite())
                    .unwrap_or(fallback)
            };
            let playing = playing_prop
                .as_ref()
                .is_none_or(|get| get(&ctx) == crate::protocol::Value::Bool(true));
            let speed = number(&speed_prop, 1.0);
            let restart = number(&restart_prop, 0.0);
            {
                let mut s = st.borrow_mut();
                s.playing = playing;
                s.speed = if speed > 0.0 { speed } else { 1.0 };
                s.restart = restart;
                s.render_into(&pic_ref, monotonic_secs());
            }
            ensure_tick(&st, &pic_ref);
        });
    }

    if props.interactive {
        let click = gtk::GestureClick::new();
        {
            let st = Rc::clone(&state);
            let pic_ref = pic.clone();
            click.connect_pressed(move |_, n_press, x, y| {
                let mut s = st.borrow_mut();
                let (w, h) = (s.sizes.0.max(1.0), s.sizes.1.max(1.0));
                let (nx, ny) = ((x / w) as f32, (y / h) as f32);
                if let Some(ref mut player) = s.player {
                    let _ = player.pointer(PointerEvent::Down(nx, ny));
                    if n_press == 2 {
                        let out = player.pointer(PointerEvent::DoubleClick(nx, ny));
                        if out.active {
                            s.active = true;
                        }
                    } else if n_press == 1 {
                        let out = player.pointer(PointerEvent::Click(nx, ny));
                        if out.active {
                            s.active = true;
                        }
                    }
                }
                s.render_into(&pic_ref, monotonic_secs());
                drop(s);
                ensure_tick(&st, &pic_ref);
            });
        }
        {
            let st = Rc::clone(&state);
            let pic_ref = pic.clone();
            click.connect_released(move |_, _, x, y| {
                let mut s = st.borrow_mut();
                let (w, h) = (s.sizes.0.max(1.0), s.sizes.1.max(1.0));
                if let Some(ref mut player) = s.player {
                    let _ = player.pointer(PointerEvent::Up((x / w) as f32, (y / h) as f32));
                }
                s.render_into(&pic_ref, monotonic_secs());
                drop(s);
                ensure_tick(&st, &pic_ref);
            });
        }
        pic.add_controller(click);

        let motion = gtk::EventControllerMotion::new();
        {
            let st = Rc::clone(&state);
            let pic_ref = pic.clone();
            motion.connect_motion(move |_, x, y| {
                let mut s = st.borrow_mut();
                let (w, h) = (s.sizes.0.max(1.0), s.sizes.1.max(1.0));
                let changed = s.player.as_mut().is_some_and(|p| {
                    p.pointer(PointerEvent::Move((x / w) as f32, (y / h) as f32))
                        .changed
                });
                if changed {
                    s.render_into(&pic_ref, monotonic_secs());
                }
                drop(s);
                ensure_tick(&st, &pic_ref);
            });
        }
        {
            let st = Rc::clone(&state);
            let pic_ref = pic.clone();
            motion.connect_leave(move |_| {
                let mut s = st.borrow_mut();
                let changed = s
                    .player
                    .as_mut()
                    .is_some_and(|p| p.pointer(PointerEvent::Leave).changed);
                if changed {
                    s.render_into(&pic_ref, monotonic_secs());
                }
                drop(s);
                ensure_tick(&st, &pic_ref);
            });
        }
        pic.add_controller(motion);
    }

    {
        let st = Rc::clone(&state);
        pic.connect_map(move |pic| {
            st.borrow_mut().render_into(pic, monotonic_secs());
            ensure_tick(&st, pic);
        });
    }
    {
        let st = Rc::clone(&state);
        pic.connect_scale_factor_notify(move |pic| {
            st.borrow_mut().render_into(pic, monotonic_secs());
        });
    }
    {
        let st = Rc::clone(&state);
        on_cleanup(move || {
            let mut s = st.borrow_mut();
            if let Some(id) = s.tick_id.take() {
                id.remove();
            }
            s.player = None;
        });
    }

    crate::widgets::place(p, &frame);
}

#[cfg(feature = "webpage")]
mod webpage {
    use super::*;
    use webkit6::prelude::*;

    pub struct WebPageProps {
        pub url: Option<Str>,
        pub cookies_file: Option<Str>,
        pub cookie_domain: Option<Str>,
        pub user_agent: Option<Str>,
        pub close_on: Option<Str>,
        pub label: Option<Str>,
    }

    #[allow(non_snake_case)]
    pub fn WebPage(p: &Parent, ctx: &Ctx, props: WebPageProps, action: Option<crate::Act>) {
        let text = |value: &Option<Str>| {
            value
                .as_ref()
                .map(|get| get(ctx).to_string())
                .unwrap_or_default()
        };
        let session = webkit6::NetworkSession::new_ephemeral();
        let (file, domain) = (text(&props.cookies_file), text(&props.cookie_domain));
        if let (Some(manager), false) = (session.cookie_manager(), domain.is_empty()) {
            for part in std::fs::read_to_string(&file)
                .unwrap_or_default()
                .split(';')
            {
                let Some((name, value)) = part.trim().split_once('=') else {
                    continue;
                };
                if matches!(
                    name.to_lowercase().as_str(),
                    "path" | "domain" | "expires" | "max-age"
                ) {
                    continue;
                }
                let mut cookie = webkit6::soup::Cookie::new(name, value, &domain, "/", -1);
                cookie.set_secure(true);
                manager.add_cookie(&cookie, None::<&gio::Cancellable>, |_| {});
            }
        }
        let manager = webkit6::UserContentManager::new();
        let selector = text(&props.close_on);
        if !selector.is_empty() {
            let source = format!(
                "document.addEventListener('click',function(e){{var t=e.target;if(t&&t.closest&&t.closest({})){{e.stopPropagation();e.preventDefault();location.href='bern-web-page://close'}}}},true);",
                serde_json::Value::from(selector)
            );
            manager.add_script(&webkit6::UserScript::new(
                &source,
                webkit6::UserContentInjectedFrames::TopFrame,
                webkit6::UserScriptInjectionTime::Start,
                &[],
                &[],
            ));
        }
        let view = webkit6::WebView::builder()
            .network_session(&session)
            .user_content_manager(&manager)
            .build();
        let at = ctx.clone();
        view.connect_decide_policy(move |_, decision, kind| {
            if kind != webkit6::PolicyDecisionType::NavigationAction {
                return false;
            }
            let Some(navigation) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>()
            else {
                return false;
            };
            let uri = navigation
                .navigation_action()
                .and_then(|action| action.request())
                .and_then(|request| request.uri())
                .unwrap_or_default();
            if uri.starts_with("http") || uri.starts_with("about:") {
                return false;
            }
            if uri.starts_with("bern-web-page:")
                && let Some(action) = &action
            {
                action(&at);
            }
            decision.ignore();
            true
        });
        view.set_hexpand(true);
        view.set_vexpand(true);
        if let Some(settings) = WebViewExt::settings(&view) {
            settings.set_enable_javascript(true);
            let agent = text(&props.user_agent);
            if !agent.is_empty() {
                settings.set_user_agent(Some(agent.as_str()));
            }
        }
        if let Some(label) = &props.label {
            view.update_property(&[gtk::accessible::Property::Label(&label(ctx))]);
        }
        let url = text(&props.url);
        if !url.is_empty() {
            view.load_uri(&url);
        }
        crate::widgets::place(p, &view);
        let shown = view.clone();
        on_cleanup(move || shown.stop_loading());
    }
}

#[cfg(feature = "webpage")]
pub use webpage::{WebPage, WebPageProps};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_fit_keywords() {
        assert_eq!(resolve_fit(None), Fit::Contain);
        assert_eq!(resolve_fit(Some("contain")), Fit::Contain);
        assert_eq!(resolve_fit(Some("cover")), Fit::Cover);
        assert_eq!(resolve_fit(Some("fill")), Fit::Fill);
    }
}
