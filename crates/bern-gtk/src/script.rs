//! Lazy WebKit document, present only for targets selecting WebView JS.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use webkit6::prelude::*;
struct Host {
    view: Option<webkit6::WebView>,
    ready: Rc<Cell<bool>>,
    pending: Vec<String>,
}
thread_local! { static HOST: RefCell<Option<(Rc<RefCell<Host>>, glib::SourceId)>> = const { RefCell::new(None) }; }
impl Host {
    fn open(&mut self) {
        let manager = webkit6::UserContentManager::new();
        manager.register_script_message_handler("bernScript", None);
        manager.connect_script_message_received(Some("bernScript"), |_, value| {
            bern_script::host::complete(&value.to_string());
        });
        let view = webkit6::WebView::builder()
            .user_content_manager(&manager)
            .build();
        let loaded = self.ready.clone();
        view.connect_load_changed(move |_, event| {
            if event == webkit6::LoadEvent::Finished {
                loaded.set(true);
            }
        });
        view.connect_web_process_terminated(|_, _| bern_script::host::stop());
        view.load_html(&format!("<!doctype html><body><script>function bernScriptComplete(value){{window.webkit.messageHandlers.bernScript.postMessage(value)}}\n{}</script>", bern_script::HOST_RUNTIME), None);
        self.view = Some(view);
    }
    fn pump(&mut self, now: f64) {
        let batch = bern_script::host::poll(now);
        if !batch.is_empty() {
            self.pending.push(batch);
        }
        if self.view.is_none() && !self.pending.is_empty() {
            self.open();
        }
        if self.ready.get()
            && let Some(view) = &self.view
        {
            for batch in self.pending.drain(..) {
                view.evaluate_javascript(
                    &format!("bernScriptDispatch({batch})"),
                    None,
                    None,
                    None::<&gio::Cancellable>,
                    |result| {
                        if result.is_err() {
                            bern_script::host::stop();
                        }
                    },
                );
            }
        }
    }
}
pub fn start() {
    stop();
    bern_script::host::start();
    let host = Rc::new(RefCell::new(Host {
        view: None,
        ready: Rc::new(Cell::new(false)),
        pending: Vec::new(),
    }));
    let pump = host.clone();
    let clock = std::time::Instant::now();
    let timer = glib::timeout_add_local(Duration::from_millis(16), move || {
        pump.borrow_mut()
            .pump(clock.elapsed().as_secs_f64() * 1000.0);
        glib::ControlFlow::Continue
    });
    HOST.with(|state| *state.borrow_mut() = Some((host, timer)));
}
pub fn stop() {
    HOST.with(|state| {
        if let Some((host, timer)) = state.borrow_mut().take() {
            timer.remove();
            if let Some(view) = &host.borrow().view {
                view.stop_loading();
            }
        }
    });
    bern_script::host::stop();
}
