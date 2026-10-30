//! Lengths in points, scaled inside a `ScaleBox`: content built while a
//! scene's scale is in effect measures everything in that scale, as the
//! scene itself is fitted to the box.

use gpui::Pixels;
use std::cell::Cell;

thread_local! {
    static SCALE: Cell<f32> = const { Cell::new(1.0) };
}

/// `value` points at the current scale.
pub(crate) fn px(value: f32) -> Pixels {
    gpui::px(value * SCALE.with(Cell::get))
}

/// The scale content is being built at.
pub(crate) fn scale() -> f32 {
    SCALE.with(Cell::get)
}

/// Build with lengths `factor` times larger than around it.
pub(crate) fn scaled<R>(factor: f32, build: impl FnOnce() -> R) -> R {
    let outer = scale();
    SCALE.with(|scale| scale.set(outer * factor));
    let result = build();
    SCALE.with(|scale| scale.set(outer));
    result
}
