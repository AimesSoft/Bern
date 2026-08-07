//! Shared runtime UI state.
//!
//! - [`PressOrigin`] lets interactive widgets record where a press happened,
//!   so background effects (like the theme-switch circular reveal) can start
//!   from the button that triggered the color change.
//! - [`ThemeReveal`] is the notification hub for the two-phase theme switch:
//!   the background notifies subscribed buttons as the reveal passes over
//!   them, and only after every button confirmed its underlying color changed
//!   does the app actually switch the mode. Event-driven by design — no
//!   per-frame polling.

use iced::Theme;
use std::sync::{Arc, Mutex};

/// The last recorded press position in window coordinates.
#[derive(Debug, Default, Clone)]
pub struct PressOrigin(Arc<Mutex<Option<(f32, f32)>>>);

impl PressOrigin {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a press position.
    pub fn record(&self, point: (f32, f32)) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(point);
        }
    }

    /// Reads and clears the last recorded press position.
    pub fn take(&self) -> Option<(f32, f32)> {
        self.0.lock().ok().and_then(|mut slot| slot.take())
    }
}

/// Notification hub for circular theme reveals.
#[derive(Debug, Default, Clone)]
pub struct ThemeReveal(Arc<Mutex<RevealInner>>);

#[derive(Debug, Default)]
struct RevealInner {
    active: bool,
    epoch: u64,
    target: Option<Theme>,
    origin: (f32, f32),
    subscribers: Vec<Subscriber>,
    next_id: u64,
}

#[derive(Debug, Clone, Copy)]
struct Subscriber {
    id: u64,
    position: (f32, f32),
    covered: bool,
    command: bool,
}

impl ThemeReveal {
    /// Creates an empty coordinator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a reveal toward `target` from `origin`. Returns `false` if one
    /// is already running (the request is ignored).
    pub fn begin(&self, target: Theme, origin: (f32, f32)) -> bool {
        let mut inner = self.0.lock().unwrap();
        if inner.active {
            return false;
        }
        inner.active = true;
        inner.epoch += 1;
        inner.target = Some(target);
        inner.origin = origin;
        inner.subscribers.clear();
        inner.next_id = 0;
        true
    }

    /// Ends the current reveal.
    pub fn finish(&self) {
        let mut inner = self.0.lock().unwrap();
        inner.active = false;
        inner.target = None;
        inner.subscribers.clear();
    }

    /// Whether a reveal is currently running.
    pub fn is_active(&self) -> bool {
        self.0.lock().unwrap().active
    }

    /// The generation of the current reveal; widgets re-subscribe when it
    /// changes.
    pub fn epoch(&self) -> u64 {
        self.0.lock().unwrap().epoch
    }

    /// The theme being revealed, if a reveal is running.
    pub fn target(&self) -> Option<Theme> {
        self.0.lock().unwrap().target.clone()
    }

    /// The origin of the running reveal.
    pub fn origin(&self) -> Option<(f32, f32)> {
        let inner = self.0.lock().unwrap();
        if inner.active {
            Some(inner.origin)
        } else {
            None
        }
    }

    /// Number of interactive widgets currently subscribed.
    pub fn subscriber_count(&self) -> usize {
        self.0.lock().unwrap().subscribers.len()
    }

    /// Number of subscribers the sweep has covered so far.
    pub fn covered_count(&self) -> usize {
        self.0
            .lock()
            .unwrap()
            .subscribers
            .iter()
            .filter(|s| s.covered)
            .count()
    }

    /// Positions of all current subscribers (debug aid).
    pub fn positions(&self) -> Vec<(f32, f32)> {
        self.0
            .lock()
            .unwrap()
            .subscribers
            .iter()
            .map(|s| s.position)
            .collect()
    }

    /// Registers an interactive widget position for the current reveal and
    /// returns its subscriber id.
    pub fn subscribe(&self, position: (f32, f32)) -> u64 {
        let mut inner = self.0.lock().unwrap();
        let id = inner.next_id;
        inner.next_id += 1;
        inner.subscribers.push(Subscriber {
            id,
            position,
            covered: false,
            command: false,
        });
        id
    }

    /// Updates a subscriber's position (called on every event pass).
    pub fn update_position(&self, id: u64, position: (f32, f32)) {
        let mut inner = self.0.lock().unwrap();
        if let Some(subscriber) = inner.subscribers.iter_mut().find(|s| s.id == id) {
            subscriber.position = position;
        }
    }

    /// Marks subscribers as covered by the sweep.
    ///
    /// With `outside == false` a subscriber is covered once the expanding
    /// circle reaches it (dark reveal). With `outside == true` a subscriber
    /// is covered once the shrinking circle no longer covers it (light
    /// reveal). Every newly covered subscriber receives a one-shot command.
    /// Returns `true` on the transition where all subscribers are covered.
    pub fn notify_covered(&self, origin: (f32, f32), radius: f32, outside: bool) -> bool {
        let mut inner = self.0.lock().unwrap();
        let mut any_new = false;
        for subscriber in &mut inner.subscribers {
            if !subscriber.covered {
                let dx = subscriber.position.0 - origin.0;
                let dy = subscriber.position.1 - origin.1;
                let distance = (dx * dx + dy * dy).sqrt();
                let reached = if outside {
                    distance >= radius
                } else {
                    distance <= radius
                };
                if reached {
                    subscriber.covered = true;
                    subscriber.command = true;
                    any_new = true;
                }
            }
        }
        any_new && inner.subscribers.iter().all(|s| s.covered)
    }

    /// Delivers the coverage command to a subscriber exactly once. Returns
    /// `true` when the sweep has just reached this subscriber's position.
    pub fn take_command(&self, id: u64) -> bool {
        let mut inner = self.0.lock().unwrap();
        if let Some(subscriber) = inner.subscribers.iter_mut().find(|s| s.id == id)
            && subscriber.command
        {
            subscriber.command = false;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_takes_once() {
        let origin = PressOrigin::new();
        assert_eq!(origin.take(), None);
        origin.record((12.0, 34.0));
        assert_eq!(origin.take(), Some((12.0, 34.0)));
        assert_eq!(origin.take(), None, "take clears the value");
    }

    #[test]
    fn reveal_notifies_when_all_subscribers_covered() {
        let reveal = ThemeReveal::new();
        assert!(!reveal.is_active());

        assert!(reveal.begin(Theme::Dark, (10.0, 10.0)));
        assert!(
            !reveal.begin(Theme::Light, (0.0, 0.0)),
            "a second begin while active is ignored"
        );
        assert_eq!(reveal.target(), Some(Theme::Dark));
        assert_eq!(reveal.origin(), Some((10.0, 10.0)));

        let near = reveal.subscribe((20.0, 10.0));
        let far = reveal.subscribe((100.0, 100.0));
        assert_eq!(reveal.subscriber_count(), 2);

        // A tiny circle covers nothing.
        assert!(!reveal.notify_covered((10.0, 10.0), 5.0, false));
        // Covers only the near button.
        assert!(!reveal.notify_covered((10.0, 10.0), 15.0, false));
        // Covers the far button too: the all-covered transition fires once.
        assert!(reveal.notify_covered((10.0, 10.0), 200.0, false));
        assert!(!reveal.notify_covered((10.0, 10.0), 200.0, false));

        // The near button received exactly one command.
        assert!(reveal.take_command(near), "first take delivers the command");
        assert!(!reveal.take_command(near), "command is consumed once");

        reveal.update_position(near, (30.0, 30.0));
        reveal.finish();
        assert!(!reveal.is_active());
        assert_eq!(reveal.origin(), None);
        assert_eq!(reveal.target(), None);
        let _ = far;
    }

    #[test]
    fn reverse_sweep_commands_outside_circle() {
        let reveal = ThemeReveal::new();
        reveal.begin(Theme::Light, (0.0, 0.0));
        let far = reveal.subscribe((100.0, 0.0));
        let at_origin = reveal.subscribe((0.0, 0.0));

        // Large circle: nothing is outside yet (reverse sweep).
        assert!(!reveal.notify_covered((0.0, 0.0), 200.0, true));
        // Shrinking circle: the far button is now outside -> command.
        assert!(!reveal.notify_covered((0.0, 0.0), 50.0, true));
        assert!(reveal.take_command(far));
        // The origin button is only covered when the circle is gone.
        assert!(reveal.notify_covered((0.0, 0.0), 0.0, true));
        assert!(reveal.take_command(at_origin));
        assert!(!reveal.take_command(at_origin));
    }
}
