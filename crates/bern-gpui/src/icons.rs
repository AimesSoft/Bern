//! Icons of the Bern icon set, drawn from the SVG documents the build put
//! in the application (Lucide's, as the Web target draws them). GPUI draws
//! an SVG as a mask in the color it is given, as a template image.

use crate::units::px;
use gpui::{AssetSource, IntoElement, Rgba, SharedString, Styled, svg};
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
    svg()
        .path(SharedString::from(format!("icons/{name}.svg")))
        .size(px(size))
        .flex_none()
        .text_color(color)
        .into_any_element()
}
