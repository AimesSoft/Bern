//! Engine for the native controls showcase.

use bern::CommandError;
use native_controls_protocol::native_controls::{self, State};

const DEFAULT_DATE: &str = "2026-11-19";

pub struct NativeControlsEngine {
    state: State,
}

impl Default for NativeControlsEngine {
    fn default() -> Self {
        Self {
            state: State {
                level: 6.0,
                boosted: false,
                selected_date: DEFAULT_DATE.to_string(),
            },
        }
    }
}

impl native_controls::Commands for NativeControlsEngine {
    fn set_level(&mut self, level: f64) -> Result<(), CommandError> {
        if !level.is_finite() {
            return Err(CommandError::invalid_params("level must be finite"));
        }
        self.state.level = (level.clamp(0.0, 10.0) * 10.0).round() / 10.0;
        Ok(())
    }

    fn set_boosted(&mut self, boosted: bool) -> Result<(), CommandError> {
        self.state.boosted = boosted;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), CommandError> {
        self.state.level = 5.0;
        self.state.boosted = false;
        Ok(())
    }

    fn set_date(&mut self, date: String) -> Result<(), CommandError> {
        let trimmed = date.trim();
        if trimmed.len() == 10
            && trimmed.as_bytes()[4] == b'-'
            && trimmed.as_bytes()[7] == b'-'
            && trimmed
                .bytes()
                .enumerate()
                .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        {
            self.state.selected_date = trimmed.to_string();
            Ok(())
        } else {
            Err(CommandError::invalid_params("date must use YYYY-MM-DD"))
        }
    }

    fn pick_today(&mut self) -> Result<(), CommandError> {
        self.state.selected_date = DEFAULT_DATE.to_string();
        Ok(())
    }
}

impl native_controls::Engine for NativeControlsEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
