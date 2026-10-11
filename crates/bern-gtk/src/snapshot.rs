//! Development aids, off by default: `BERN_APPEARANCE=light|dark`
//! overrides the system appearance, and `BERN_SNAPSHOT_DIR` makes the
//! application render each of its windows to `<dir>/<window>.png` after it
//! opens and after every change, for checks without screen access.

use crate::windows::{self, Win};
use adw::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

thread_local! {
    static QUEUED: Cell<bool> = const { Cell::new(false) };
}

fn directory() -> Option<PathBuf> {
    std::env::var_os("BERN_SNAPSHOT_DIR").map(PathBuf::from)
}

pub(crate) fn apply_appearance() {
    let scheme = match std::env::var("BERN_APPEARANCE").as_deref() {
        Ok("light") => adw::ColorScheme::ForceLight,
        Ok("dark") => adw::ColorScheme::ForceDark,
        _ => return,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

/// Capture every window once the current changes have been drawn.
pub(crate) fn schedule() {
    if directory().is_none() || QUEUED.with(|queued| queued.replace(true)) {
        return;
    }
    glib::timeout_add_local_once(Duration::from_millis(700), || {
        QUEUED.with(|queued| queued.set(false));
        capture_all();
    });
}

/// Capture a new window after it is shown.
pub(crate) fn watch(win: &Rc<Win>) {
    if directory().is_none() {
        return;
    }
    win.window.connect_map(|_| schedule());
    schedule();
}

fn capture_all() {
    let Some(directory) = directory() else { return };
    let _ = std::fs::create_dir_all(&directory);
    for win in windows::all() {
        let path = directory.join(format!("{}.png", win.id.replace('#', "-")));
        if let Err(problem) = capture(&win.window, &path) {
            eprintln!("bern: snapshot of {}: {problem}", win.id);
        }
    }
}

fn capture(window: &adw::ApplicationWindow, path: &std::path::Path) -> Result<(), String> {
    let (width, height) = (window.width(), window.height());
    if width <= 0 || height <= 0 {
        return Err("not shown yet".into());
    }
    let paintable = gtk::WidgetPaintable::new(Some(window));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, width as f64, height as f64);
    let node = snapshot.to_node().ok_or("nothing drawn")?;
    let renderer = window
        .native()
        .and_then(|native| native.renderer())
        .ok_or("no renderer")?;
    let texture = renderer.render_texture(
        &node,
        Some(&gtk::graphene::Rect::new(
            0.0,
            0.0,
            width as f32,
            height as f32,
        )),
    );
    texture.save_to_png(path).map_err(|error| error.to_string())
}
