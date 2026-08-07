//! The theme router: the single runtime source of the active iced theme.
//!
//! Light/dark colors are **not** configured anywhere external — every control
//! keeps its palettes in its own code and reads the routed theme through
//! [`BuildContext::theme`](crate::core::widget::BuildContext::theme). The
//! router is the only place the app switches the scheme at runtime.

use iced::Theme;

/// Routes the active `iced::Theme` to every control at build time.
#[derive(Debug, Clone)]
pub struct ThemeRouter {
    theme: Theme,
}

impl ThemeRouter {
    /// Creates a router with the given initial theme.
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }

    /// The currently routed theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Whether the routed theme is a dark scheme.
    pub fn is_dark(&self) -> bool {
        self.theme.extended_palette().is_dark
    }

    /// Routes a new theme.
    pub fn set(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Toggles between light and dark.
    pub fn toggle(&mut self) {
        self.theme = if self.is_dark() {
            Theme::Light
        } else {
            Theme::Dark
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_toggles_light_and_dark() {
        let mut router = ThemeRouter::new(Theme::Light);
        assert!(!router.is_dark());
        router.toggle();
        assert!(router.is_dark());
        assert_eq!(router.theme(), &Theme::Dark);
        router.toggle();
        assert_eq!(router.theme(), &Theme::Light);
    }
}
