//! Engine of the planner shell.

use bern::CommandError;
use planner_protocol::planner::{self, State};

pub struct PlannerEngine {
    state: State,
}

impl Default for PlannerEngine {
    fn default() -> Self {
        Self {
            state: State {
                title: "Plan".into(),
                activity: vec![2, 5, 3, 8, 6, 9, 4],
            },
        }
    }
}

impl planner::Commands for PlannerEngine {
    fn rename(&mut self, title: String) -> Result<(), CommandError> {
        if title.trim().is_empty() {
            return Err(CommandError::new("empty_title", "a plan needs a title"));
        }
        self.state.title = title;
        Ok(())
    }

    fn record(&mut self, amount: i64) -> Result<(), CommandError> {
        self.state.activity.push(amount.max(0));
        let extra = self.state.activity.len().saturating_sub(12);
        self.state.activity.drain(..extra);
        Ok(())
    }
}

impl planner::Engine for PlannerEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
