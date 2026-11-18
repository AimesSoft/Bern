//! Optional native controls anchored to a free overlay. The application owns
//! their values; the host owns placement, focus and in-progress gestures.

use super::{BTreeMap, Mutex, host};

/// The treatment of overlay text, including annotations and the lit region.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextEffect {
    None,
    #[default]
    Shadow,
    Outline,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub effect: TextEffect,
    /// Multiplies the text's original alpha without affecting overlay controls.
    pub opacity: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            effect: TextEffect::Shadow,
            opacity: 1.0,
        }
    }
}

/// The `id` of a control stays the same across updates. Colours are 24-bit
/// sRGB (`0xRRGGBB`); choices are zero-based indexes into `options`.
#[derive(Clone, Debug, PartialEq)]
pub enum Control {
    Range {
        id: String,
        label: String,
        min: f64,
        max: f64,
        step: f64,
        value: f64,
    },
    Color {
        id: String,
        label: String,
        rgb: u32,
    },
    Choice {
        id: String,
        label: String,
        options: Vec<String>,
        selected: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub title: String,
    pub controls: Vec<Control>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PanelEvent {
    Changed {
        overlay: String,
        control: String,
        value: f64,
        committed: bool,
    },
    Dismissed {
        overlay: String,
    },
}

static PANELS: Mutex<BTreeMap<String, Panel>> = Mutex::new(BTreeMap::new());
static EVENTS: Mutex<Vec<PanelEvent>> = Mutex::new(Vec::new());
static WAKERS: Mutex<BTreeMap<String, crate::wake::Waker>> = Mutex::new(BTreeMap::new());

/// Associate an owner's engine waker with its overlay controls so native UI
/// callbacks wake the engine thread immediately.
pub fn set_panel_waker(id: &str, waker: Option<crate::wake::Waker>) {
    let mut wakers = WAKERS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(waker) = waker {
        wakers.insert(id.into(), waker);
    } else {
        wakers.remove(id);
    }
}

fn wake_owner(id: &str) {
    let waker = WAKERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(id)
        .cloned();
    if let Some(waker) = waker {
        waker.wake();
    }
}

/// True for hosts that can display an interactive panel alongside a free overlay.
pub fn panel_supported() -> bool {
    host().map_or(
        cfg!(any(
            target_os = "macos",
            target_os = "windows",
            target_os = "android"
        )),
        |host| host.supports_panel(),
    )
}

/// Whether the host currently holds an open control panel for `id`.
pub fn panel_open(id: &str) -> bool {
    PANELS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key(id)
}

/// Open, update, or close the controls attached to `id`. A panel never opens
/// for an absent, locked or taskbar overlay.
pub fn show_panel(id: &str, panel: Option<&Panel>) {
    let panel = panel.filter(|_| {
        super::SHOWN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .is_some_and(|(overlay, _)| !overlay.locked && overlay.place == super::Place::Free)
    });
    {
        let mut panels = PANELS.lock().unwrap_or_else(|e| e.into_inner());
        if panels.get(id) == panel {
            return;
        }
        match panel {
            Some(panel) => {
                panels.insert(id.to_owned(), panel.clone());
            }
            None => {
                panels.remove(id);
            }
        }
    }
    match host() {
        Some(host) => host.panel(id, panel),
        None => panel_here(id, panel),
    }
}

fn panel_here(id: &str, panel: Option<&Panel>) {
    #[cfg(target_os = "macos")]
    super::mac::panel(id.to_owned(), panel.cloned());
    #[cfg(target_os = "windows")]
    super::win::panel(id.to_owned(), panel.cloned());
    #[cfg(target_os = "android")]
    {
        let data = panel.map(panel_json).unwrap_or_default();
        let _ = crate::platform::request("overlay_panel", id, &data);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
    let _ = (id, panel);
}

/// Report a native control value change, coalescing consecutive drag samples
/// while preserving commits.
pub fn panel_changed(id: &str, control: &str, value: f64, committed: bool) {
    if !value.is_finite() {
        return;
    }
    let mut events = EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    if matches!(
        events.last(),
        Some(PanelEvent::Changed {
            overlay,
            control: last,
            committed: false,
            ..
        }) if overlay == id && last == control
    ) {
        events.pop();
    }
    events.push(PanelEvent::Changed {
        overlay: id.into(),
        control: control.into(),
        value,
        committed,
    });
    drop(events);
    wake_owner(id);
}

/// Report that the host closed the panel (close button or Escape).
pub fn panel_dismissed(id: &str) {
    if PANELS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(id)
        .is_none()
    {
        return;
    }
    EVENTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(PanelEvent::Dismissed { overlay: id.into() });
    wake_owner(id);
}

/// Drain pending events belonging to `id`.
pub fn take_panel_events(id: &str) -> Vec<PanelEvent> {
    let mut events = EVENTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut taken = Vec::new();
    let mut kept = Vec::new();
    for event in events.drain(..) {
        let mine = match &event {
            PanelEvent::Changed { overlay, .. } | PanelEvent::Dismissed { overlay } => {
                overlay == id
            }
        };
        if mine {
            taken.push(event);
        } else {
            kept.push(event);
        }
    }
    *events = kept;
    taken
}

#[cfg(any(target_os = "android", test))]
pub(super) fn panel_json(panel: &Panel) -> String {
    use serde_json::json;
    let controls: Vec<_> = panel
        .controls
        .iter()
        .map(|control| match control {
            Control::Range {
                id,
                label,
                min,
                max,
                step,
                value,
            } => json!({
                "type": "range",
                "id": id,
                "label": label,
                "min": min,
                "max": max,
                "step": step,
                "value": value,
            }),
            Control::Color { id, label, rgb } => json!({
                "type": "color",
                "id": id,
                "label": label,
                "rgb": rgb,
            }),
            Control::Choice {
                id,
                label,
                options,
                selected,
            } => json!({
                "type": "choice",
                "id": id,
                "label": label,
                "options": options,
                "selected": selected,
            }),
        })
        .collect();
    json!({ "title": panel.title, "controls": controls }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_wire_values_use_stable_ids_and_rgb_order() {
        let panel = Panel {
            title: "Display".into(),
            controls: vec![Control::Color {
                id: "accent".into(),
                label: "Colour".into(),
                rgb: 0x123456,
            }],
        };
        let data: serde_json::Value = serde_json::from_str(&panel_json(&panel)).unwrap();
        assert_eq!(data["controls"][0]["id"], "accent");
        assert_eq!(data["controls"][0]["rgb"], 0x123456);
    }
}
