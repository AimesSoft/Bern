//! Icons of the Bern icon set, drawn from the SVG documents the build put
//! in the application (Lucide's, as the Web target draws them). GPUI draws
//! an SVG as a mask in the color it is given, as a template image.

use crate::units::px;
use gpui::{AssetSource, IntoElement, ParentElement, Rgba, SharedString, Styled, div, svg};
use std::borrow::Cow;

/// The application's icons as GPUI assets, at `icons/<name>.svg`.
pub(crate) struct Assets(pub(crate) &'static [(&'static str, &'static str)]);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = path
            .strip_prefix("icons/")
            .and_then(|name| name.strip_suffix(".svg"));
        Ok(name.and_then(|name| {
            self.0
                .iter()
                .find(|(icon, _)| *icon == name)
                .map(|(_, document)| Cow::Borrowed(document.as_bytes()))
        }))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(self
            .0
            .iter()
            .map(|(name, _)| format!("icons/{name}.svg").into())
            .collect())
    }
}

/// The icon `name`, `size` points square, in `color`.
pub(crate) fn icon(name: &str, size: f32, color: Rgba) -> gpui::AnyElement {
    let blur = crate::filters::building_blur();
    if blur > 0.05 {
        return crate::filters::blurred_icon(name, size, color, blur);
    }
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}

/// Two copies of the same mask, the accent clipped from the top down.
pub(crate) fn decorated(
    name: &str,
    size: f32,
    foreground: Rgba,
    accent: Rgba,
    surface: Rgba,
    progress: Option<f32>,
    badge: Option<&str>,
) -> gpui::AnyElement {
    if progress.is_none() && badge.is_none() {
        return icon(name, size, foreground);
    }
    let fraction = progress
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let mut glyph = div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(icon(name, size, foreground));
    if fraction > 0.0 {
        glyph = glyph.child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .w(px(size))
                .h(px(size * fraction))
                .overflow_hidden()
                .child(icon(name, size, accent)),
        );
    }
    if let Some(badge) = badge {
        glyph = glyph.child(
            div()
                .absolute()
                .right(px(-size * 0.06))
                .bottom(px(-size * 0.06))
                .size(px(size * 0.52))
                .rounded(px(size))
                .bg(surface)
                .flex()
                .items_center()
                .justify_center()
                .child(icon(badge, size * 0.4, accent)),
        );
    }
    glyph.into_any_element()
}
