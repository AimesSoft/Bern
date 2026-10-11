//! Engine of the hello world.

use bern::CommandError;
use hello_protocol::hello::{self, State};

pub struct HelloEngine {
    state: State,
}

impl Default for HelloEngine {
    fn default() -> Self {
        Self {
            state: State { waves: 0 },
        }
    }
}

impl hello::Commands for HelloEngine {
    fn wave(&mut self) -> Result<(), CommandError> {
        self.state.waves += 1;
        Ok(())
    }
}

impl hello::Engine for HelloEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
