//! Engine of the {{id}} module: plain Rust, tested without a window.

use bern::CommandError;
use {{crate}}_protocol::{{id}}::{self, State};

pub struct {{engine}} {
    state: State,
}

impl Default for {{engine}} {
    fn default() -> Self {
        Self {
            state: State { count: 0 },
        }
    }
}

impl {{id}}::Commands for {{engine}} {
    fn add_one(&mut self) -> Result<(), CommandError> {
        self.state.count += 1;
        Ok(())
    }
}

impl {{id}}::Engine for {{engine}} {
    fn state(&self) -> &State {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {{id}}::Commands;

    #[test]
    fn the_button_counts() {
        let mut engine = {{engine}}::default();
        engine.add_one().unwrap();
        assert_eq!(engine.state.count, 1);
    }
}
