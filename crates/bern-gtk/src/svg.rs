//! GTK presenter for bern-svg, enabled only in applications using Svg.
use crate::reactive::{effect, on_cleanup};
use crate::{Ctx, Get, Num, Parent, Str};
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use webkit6::prelude::*;

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

/// The presenter's update for the bound values: a speed that is not
/// positive plays at 1, and the picture fits `contain` unless declared.
fn update_script(
    playing: bool,
    speed: f64,
    restart: f64,
    fit: Option<&str>,
    interactive: bool,
    data: &str,
    external_links: bool,
) -> String {
    let update = serde_json::json!({"playing": playing, "speed": if speed > 0.0 { speed } else { 1.0 },
        "restart": restart, "fit": fit.unwrap_or("contain"), "interactive": interactive, "data": data, "externalLinks": external_links});
    format!("window.bernSvg?.update({update})")
}

fn evaluate(view: &webkit6::WebView, script: &str) {
    view.evaluate_javascript(script, None, None, None::<&gio::Cancellable>, |result| {
        if let Err(error) = result {
            eprintln!("Bern SVG: {error}");
        }
    });
}

#[allow(non_snake_case)]
pub fn Svg(p: &Parent, ctx: &Ctx, props: SvgProps) {
    let view = webkit6::WebView::new();
    view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
    if let Some(settings) = WebViewExt::settings(&view) {
        settings.set_enable_javascript(true);
        settings.set_javascript_can_open_windows_automatically(false);
        settings.set_enable_html5_local_storage(false);
    }
    let ready = Rc::new(Cell::new(false));
    let state = Rc::new(RefCell::new(String::new()));
    let loaded = Rc::clone(&ready);
    let latest = Rc::clone(&state);
    view.connect_load_changed(move |view, event| {
        if event == webkit6::LoadEvent::Finished {
            loaded.set(true);
            evaluate(view, &latest.borrow());
            evaluate(
                view,
                if view.is_mapped() {
                    "window.bernSvg?.update({visible:true})"
                } else {
                    "window.bernSvg?.update({visible:false})"
                },
            );
        }
    });
    let external_links = props.external_links;
    view.connect_decide_policy(move |_, decision, kind| {
        if (kind == webkit6::PolicyDecisionType::NavigationAction
            || kind == webkit6::PolicyDecisionType::NewWindowAction)
            && let Some(nav) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>()
        {
            let uri = nav
                .navigation_action()
                .and_then(|action| action.request())
                .and_then(|req| req.uri());
            if external_links
                && nav
                    .navigation_action()
                    .is_some_and(|a| a.navigation_type() == webkit6::NavigationType::LinkClicked)
                && let Some(url) = uri
                    .as_deref()
                    .filter(|u| u.starts_with("https://") && !u.chars().any(char::is_whitespace))
                && let Err(error) =
                    gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>)
            {
                eprintln!("Open SVG link: {error}");
            }
            if uri.as_deref() != Some("about:blank") {
                decision.ignore();
                return true;
            }
        }
        false
    });
    let frame = crate::picture::Frame::new(&view, 0, 0);
    let sizes = Rc::new(Cell::new((0, 0)));
    if let Some(width) = &props.width {
        let sizes = sizes.clone();
        let frame = frame.clone();
        crate::motion::bind_number(ctx, width, None, &view, move |n| {
            sizes.set((n.round() as i32, sizes.get().1));
            frame.set_size(sizes.get().0, sizes.get().1);
        });
    }
    if let Some(height) = &props.height {
        let sizes = sizes.clone();
        let frame = frame.clone();
        crate::motion::bind_number(ctx, height, None, &view, move |n| {
            sizes.set((sizes.get().0, n.round() as i32));
            frame.set_size(sizes.get().0, sizes.get().1);
        });
    }
    if let Some(label) = props.label {
        let ctx = ctx.clone();
        let view = view.clone();
        effect(move || view.update_property(&[gtk::accessible::Property::Label(&label(&ctx))]));
    } else {
        view.update_state(&[gtk::accessible::State::Hidden(true)]);
    }
    let ctx = ctx.clone();
    let displayed = view.clone();
    effect(move || {
        let number = |get: &Option<Get>, fallback| {
            get.as_ref()
                .map(|get| crate::value::as_f64(&get(&ctx)))
                .filter(|n| n.is_finite())
                .unwrap_or(fallback)
        };
        let playing = props
            .playing
            .as_ref()
            .is_none_or(|get| get(&ctx) == crate::protocol::Value::Bool(true));
        let speed = number(&props.speed, 1.0);
        let restart = number(&props.restart, 0.0);
        *state.borrow_mut() = update_script(
            playing,
            speed,
            restart,
            props.fit,
            props.interactive,
            &props.data.as_ref().map(|get| get(&ctx)).unwrap_or_default(),
            props.external_links,
        );
        if ready.get() {
            evaluate(&displayed, &state.borrow());
        }
    });
    view.connect_map(|view| evaluate(view, "window.bernSvg?.update({visible:true})"));
    view.connect_unmap(|view| evaluate(view, "window.bernSvg?.update({visible:false})"));
    let source = std::path::Path::new(&crate::host::resources_dir()).join(props.source);
    match std::fs::read_to_string(&source)
        .map_err(|e| e.to_string())
        .and_then(|s| bern_svg::document(&s))
    {
        Ok(html) => view.load_html(&html, None),
        Err(error) => eprintln!("Bern SVG {}: {error}", source.display()),
    }
    let closing = view.clone();
    on_cleanup(move || {
        closing.stop_loading();
        closing.load_html("", None);
    });
    crate::widgets::place(p, &frame);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The values an update script passes to the presenter.
    fn values(script: &str) -> serde_json::Value {
        let json = script
            .strip_prefix("window.bernSvg?.update(")
            .and_then(|rest| rest.strip_suffix(')'))
            .expect("an update call");
        serde_json::from_str(json).expect("JSON")
    }

    #[test]
    fn updates_pass_the_bound_values() {
        assert_eq!(
            values(&update_script(
                false,
                2.0,
                1.5,
                Some("cover"),
                true,
                "{}",
                false
            )),
            serde_json::json!({"playing": false, "speed": 2.0, "restart": 1.5, "fit": "cover", "interactive": true, "data": "{}", "externalLinks": false})
        );
    }

    #[test]
    fn speeds_that_are_not_positive_play_at_one() {
        for speed in [0.0, -1.0] {
            let update = values(&update_script(true, speed, 0.0, None, false, "", false));
            assert_eq!(update["speed"], 1.0);
        }
        let update = values(&update_script(true, 1.0, 0.0, None, false, "", false));
        assert_eq!(update["fit"], "contain");
    }
}
