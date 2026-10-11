//! The engine of {{name}}: the business logic, in plain Rust. It knows
//! nothing about windows, so it is tested without one.

use bern::CommandError;
use {{id}}_protocol::{{id}}::{self, State};

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

    fn reset(&mut self) -> Result<(), CommandError> {
        self.state.count = 0;
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
    fn the_button_counts_and_reset_starts_over() {
        let mut engine = {{engine}}::default();
        engine.add_one().unwrap();
        engine.add_one().unwrap();
        assert_eq!(engine.state.count, 2);
        engine.reset().unwrap();
        assert_eq!(engine.state.count, 0);
    }
}
