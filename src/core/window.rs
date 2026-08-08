//! Framework-level native window configuration.
//!
//! Bern keeps native chrome decisions separate from RON page layouts: the
//! application chooses them once while constructing its iced application.

/// A native-window command emitted by the [`window_controls`](crate::widgets::window_controls)
/// widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    /// Minimize the latest application window.
    Minimize,
    /// Toggle the latest application window between maximized and restored.
    ToggleMaximize,
    /// Close the latest application window.
    Close,
}

/// Converts a [`WindowControlAction`] into the matching iced runtime task.
///
/// Applications return this task from `update` after receiving
/// [`EventKind::WindowControl`](crate::EventKind::WindowControl). Keeping the
/// task here makes the RON control reusable without binding Bern's widget
/// layer to one application message type.
pub fn perform_window_control_action<Message>(
    action: WindowControlAction,
) -> iced::Task<Message>
where
    Message: Send + 'static,
{
    match action {
        WindowControlAction::Minimize => {
            iced::window::latest().and_then(|id| iced::window::minimize(id, true))
        }
        WindowControlAction::ToggleMaximize => iced::window::latest().and_then(|id| {
            iced::window::is_maximized(id)
                .then(move |maximized| iced::window::maximize(id, !maximized))
        }),
        WindowControlAction::Close => {
            iced::window::latest().and_then(iced::window::close)
        }
    }
}

/// Bern's portable native-window options.
///
/// The default preserves the operating system title bar. Use
/// [`WindowOptions::hide_title_bar`] to let application content extend into
/// the title-bar area while retaining native window shape where supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowOptions {
    hide_title_bar: bool,
}

impl WindowOptions {
    /// Creates options with the native title bar visible.
    pub const fn new() -> Self {
        Self {
            hide_title_bar: false,
        }
    }

    /// Chooses whether the native title bar is visually hidden.
    #[must_use]
    pub const fn hide_title_bar(mut self, hide: bool) -> Self {
        self.hide_title_bar = hide;
        self
    }

    /// Returns whether the title bar is visually hidden.
    pub const fn is_title_bar_hidden(self) -> bool {
        self.hide_title_bar
    }

    /// Applies Bern's chrome options to existing iced window settings.
    ///
    /// Other settings such as dimensions, resize behavior, and transparency
    /// are preserved.
    pub fn apply_to(self, mut settings: iced::window::Settings) -> iced::window::Settings {
        #[cfg(target_os = "macos")]
        {
            // Do not disable decorations on macOS: doing so selects a fully
            // borderless NSWindow style and loses AppKit's rounded corners
            // and native shadow. A transparent, full-size title bar gives us
            // the desired visual result while preserving native chrome.
            settings.decorations = true;
            settings.platform_specific.title_hidden = self.hide_title_bar;
            settings.platform_specific.titlebar_transparent = self.hide_title_bar;
            settings.platform_specific.fullsize_content_view = self.hide_title_bar;
        }

        #[cfg(not(target_os = "macos"))]
        {
            // iced currently exposes no portable transparent-title-bar API.
            // Other platforms use undecorated windows as the closest match.
            settings.decorations = !self.hide_title_bar;
        }
        settings
    }

    /// Produces default iced window settings with these options applied.
    pub fn into_settings(self) -> iced::window::Settings {
        self.apply_to(iced::window::Settings::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_bar_is_visible_by_default() {
        let options = WindowOptions::default();
        assert!(!options.is_title_bar_hidden());
        assert!(options.into_settings().decorations);
    }

    #[test]
    fn hidden_title_bar_uses_platform_appropriate_settings() {
        let settings = WindowOptions::new().hide_title_bar(true).into_settings();

        #[cfg(target_os = "macos")]
        {
            assert!(settings.decorations);
            assert!(settings.platform_specific.title_hidden);
            assert!(settings.platform_specific.titlebar_transparent);
            assert!(settings.platform_specific.fullsize_content_view);
        }

        #[cfg(not(target_os = "macos"))]
        assert!(!settings.decorations);
    }

    #[test]
    fn applying_chrome_preserves_other_window_settings() {
        let original = iced::window::Settings {
            size: iced::Size::new(1280.0, 720.0),
            resizable: false,
            ..Default::default()
        };
        let settings = WindowOptions::new()
            .hide_title_bar(true)
            .apply_to(original);

        assert_eq!(settings.size, iced::Size::new(1280.0, 720.0));
        assert!(!settings.resizable);
        #[cfg(target_os = "macos")]
        assert!(settings.decorations);
        #[cfg(not(target_os = "macos"))]
        assert!(!settings.decorations);
    }
}
