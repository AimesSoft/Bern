//! Input arbitration independent of GTK, exercised by host-side regressions.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Tap {
    None,
    Single,
    Pending,
    Double,
}

#[derive(Default)]
pub(crate) struct Taps {
    pub generation: u64,
    down: bool,
    second: bool,
    pending: bool,
    at: u64,
    x: f64,
    y: f64,
}

impl Taps {
    /// Returns whether an earlier, unrelated single click must be delivered.
    pub fn press(&mut self, now: u64, x: f64, y: f64, interval: u64, distance: f64) -> bool {
        let second = self.pending
            && now.saturating_sub(self.at) <= interval
            && (x - self.x).abs() <= distance
            && (y - self.y).abs() <= distance;
        let flush = self.pending && !second;
        self.generation += 1;
        self.pending = false;
        self.down = true;
        self.second = second;
        self.at = now;
        self.x = x;
        self.y = y;
        flush
    }

    pub fn moved(&mut self, x: f64, y: f64, threshold: f64) -> bool {
        if self.down && ((x - self.x).abs() > threshold || (y - self.y).abs() > threshold) {
            self.cancel();
            return true;
        }
        false
    }

    pub fn click(&mut self) -> Tap {
        if !self.down {
            return Tap::None;
        }
        self.down = false;
        if self.second {
            self.second = false;
            Tap::Double
        } else {
            self.pending = true;
            Tap::Pending
        }
    }

    pub fn expire(&mut self, generation: u64) -> Tap {
        if self.generation == generation && self.pending {
            self.pending = false;
            Tap::Single
        } else {
            Tap::None
        }
    }

    pub fn cancel(&mut self) {
        self.generation += 1;
        self.down = false;
        self.pending = false;
        self.second = false;
    }
}

#[derive(Default)]
pub(crate) struct Drag {
    pub moving: bool,
    tracking: bool,
    from: f64,
    last: f64,
}

impl Drag {
    pub fn begin(&mut self, value: f64) {
        self.tracking = true;
        self.moving = false;
        self.from = value;
        self.last = value;
    }

    pub fn update(
        &mut self,
        offset: (f64, f64),
        vertical: bool,
        threshold: f64,
        reach: f64,
        min: f64,
        max: f64,
    ) -> Option<f64> {
        if !self.tracking {
            return None;
        }
        let (x, y) = offset;
        let (along, across) = if vertical { (-y, x) } else { (x, y) };
        if !self.moving {
            if across.abs() > threshold && across.abs() > along.abs() {
                self.tracking = false;
                return None;
            }
            if along.abs() <= threshold || along.abs() <= across.abs() {
                return None;
            }
            self.moving = true;
        }
        let value = (self.from + along / reach.max(1.0) * (max - min))
            .max(min)
            .min(max);
        if value == self.last {
            return None;
        }
        self.last = value;
        Some(value)
    }

    pub fn end(&mut self) -> bool {
        let moved = self.moving;
        self.tracking = false;
        self.moving = false;
        moved
    }

    pub fn cancel(&mut self) {
        self.end();
    }
}

/// Keep `active` true through both actions: clearing it may synchronously
/// rebuild a Watch that writes the engine's old value into the draft.
/// The guard also releases the gesture if any callback unwinds.
pub(crate) fn finish_swipe(
    continuous: bool,
    prepare: impl FnOnce() -> bool,
    action: impl FnOnce(),
    commit: impl FnOnce(),
    cleanup: impl FnOnce(),
) {
    struct Finally<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Finally<F> {
        fn drop(&mut self) {
            if let Some(cleanup) = self.0.take() {
                cleanup();
            }
        }
    }
    let _finally = Finally(Some(cleanup));
    if prepare() {
        if !continuous {
            action();
        }
        commit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swipe_commit_precedes_active_observer_and_cleanup_survives_failure() {
        use std::cell::Cell;
        for continuous in [false, true] {
            let active = Cell::new(true);
            let draft = Cell::new(0.2);
            let actions = Cell::new(0);
            let committed = Cell::new(0.0);
            finish_swipe(
                continuous,
                || {
                    draft.set(0.8);
                    true
                },
                || {
                    assert!(active.get());
                    assert_eq!(draft.get(), 0.8);
                    actions.set(actions.get() + 1);
                },
                || {
                    assert!(active.get());
                    committed.set(draft.get());
                },
                || {
                    active.set(false);
                    draft.set(0.2);
                }, // synchronous Watch of old engine state
            );
            assert_eq!(committed.get(), 0.8);
            assert_eq!(actions.get(), if continuous { 0 } else { 1 });
            assert!(!active.get());
            assert_eq!(draft.get(), 0.2);
            active.set(true);
            finish_swipe(
                continuous,
                || false,
                || panic!("canceled action"),
                || panic!("canceled commit"),
                || active.set(false),
            );
            assert!(!active.get());
        }
        for failure in 0..3 {
            let cleaned = Cell::new(false);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                finish_swipe(
                    false,
                    || {
                        assert_ne!(failure, 0, "final update failed");
                        true
                    },
                    || assert_ne!(failure, 1, "action failed"),
                    || assert_ne!(failure, 2, "commit failed"),
                    || cleaned.set(true),
                );
            }));
            assert!(result.is_err());
            assert!(cleaned.get());
        }
    }

    #[test]
    fn single_double_drag_cancel_and_stale_timer() {
        let mut t = Taps::default();
        assert!(!t.press(10, 0.0, 0.0, 400, 5.0));
        assert_eq!(t.click(), Tap::Pending);
        let first = t.generation;
        assert!(!t.press(200, 1.0, 1.0, 400, 5.0));
        assert_eq!(t.expire(first), Tap::None);
        assert_eq!(t.click(), Tap::Double);
        assert_eq!(t.expire(t.generation), Tap::None);
        t.press(700, 0.0, 0.0, 400, 5.0);
        assert_eq!(t.click(), Tap::Pending);
        assert_eq!(t.expire(t.generation), Tap::Single);
        assert_eq!(t.expire(t.generation), Tap::None);
        t.press(900, 0.0, 0.0, 400, 5.0);
        t.click();
        assert!(t.press(950, 20.0, 0.0, 400, 5.0));
        t.moved(40.0, 0.0, 8.0);
        assert_eq!(t.click(), Tap::None);
        assert_eq!(t.expire(t.generation), Tap::None);
        t.press(1100, 0.0, 0.0, 400, 5.0);
        t.click();
        t.cancel(); // unmount/capture loss/keyboard activation
        assert_eq!(t.expire(t.generation), Tap::None);
    }

    #[test]
    fn swipe_threshold_axis_clamp_cancel_and_rebase() {
        let mut d = Drag::default();
        d.begin(0.5);
        assert_eq!(d.update((4.0, 2.0), false, 8.0, 300.0, 0.0, 1.0), None);
        assert!(!d.end()); // a click is never a drag
        d.begin(0.5);
        d.update((9.0, 20.0), false, 8.0, 300.0, 0.0, 1.0);
        d.update((50.0, 20.0), false, 8.0, 300.0, 0.0, 1.0);
        assert!(!d.end()); // perpendicular scrolling cannot later be stolen
        d.begin(0.5);
        assert_eq!(
            d.update((30.0, 3.0), false, 8.0, 300.0, 0.0, 1.0),
            Some(0.6)
        );
        assert!(d.moving);
        assert_eq!(
            d.update((600.0, 300.0), false, 8.0, 300.0, 0.0, 1.0),
            Some(1.0)
        );
        assert_eq!(d.update((700.0, 300.0), false, 8.0, 300.0, 0.0, 1.0), None);
        assert!(d.end());
        d.begin(0.2); // the next gesture starts at the externally synchronized value
        assert_eq!(
            d.update((0.0, -30.0), true, 8.0, 300.0, 0.0, 1.0),
            Some(0.2 + 0.1)
        );
        d.cancel();
        assert!(!d.end());
    }
}
