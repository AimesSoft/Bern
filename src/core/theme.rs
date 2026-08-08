//! The theme router: the single runtime source of the active iced theme.
//!
//! Light/dark colors are **not** configured anywhere external — every control
//! keeps its palettes in its own code and reads the routed theme through
//! [`BuildContext::theme`](crate::core::widget::BuildContext::theme). The
//! router is the only place the app switches the scheme at runtime.

use iced::{Color, Theme};

/// Routes the active `iced::Theme` to every control at build time.
#[derive(Debug, Clone)]
pub struct ThemeRouter {
    theme: Theme,
    accent: Color,
}

impl ThemeRouter {
    /// Creates a router with the given initial theme.
    pub fn new(theme: Theme) -> Self {
        let accent = theme.palette().primary;
        Self { theme, accent }
    }

    /// The currently routed theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Whether the routed theme is a dark scheme.
    pub fn is_dark(&self) -> bool {
        self.theme.extended_palette().is_dark
    }

    /// The primary accent shared by every routed control.
    pub fn accent(&self) -> Color {
        self.accent
    }

    /// Builds a light or dark theme while preserving the routed accent.
    pub fn theme_for(&self, dark: bool) -> Theme {
        theme_with_accent(dark, self.accent)
    }

    /// Routes a new theme.
    pub fn set(&mut self, theme: Theme) {
        self.accent = theme.palette().primary;
        self.theme = theme;
    }

    /// Changes the global accent without changing the light/dark mode.
    pub fn set_accent(&mut self, accent: Color) {
        self.accent = accent;
        self.theme = theme_with_accent(self.is_dark(), accent);
    }

    /// Toggles between light and dark.
    pub fn toggle(&mut self) {
        self.theme = self.theme_for(!self.is_dark());
    }
}

fn theme_with_accent(dark: bool, accent: Color) -> Theme {
    let mut palette = if dark {
        iced::theme::Palette::DARK
    } else {
        iced::theme::Palette::LIGHT
    };
    palette.primary = accent;
    Theme::custom(
        if dark {
            "Bern Custom Dark"
        } else {
            "Bern Custom Light"
        },
        palette,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_toggles_light_and_dark() {
        let mut router = ThemeRouter::new(Theme::Light);
        let accent = router.accent();
        assert!(!router.is_dark());
        router.toggle();
        assert!(router.is_dark());
        assert_eq!(router.theme().palette().primary, accent);
        router.toggle();
        assert!(!router.is_dark());
        assert_eq!(router.theme().palette().primary, accent);
    }
}
