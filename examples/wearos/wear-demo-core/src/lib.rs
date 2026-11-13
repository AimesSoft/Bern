use bern::CommandError;
use wear_demo_protocol::watch_demo::{self, Entry, Goal, State};

pub struct WatchEngine {
    state: State,
}

impl Default for WatchEngine {
    fn default() -> Self {
        Self {
            state: State {
                count: 0,
                reminders: false,
                level: 50.0,
                goal: Goal::Daily,
                entries: (1..=24).map(|id| Entry { id, count: id * 3 }).collect(),
            },
        }
    }
}

impl watch_demo::Commands for WatchEngine {
    fn add(&mut self) -> Result<(), CommandError> {
        self.state.count += 1;
        Ok(())
    }
    fn reset(&mut self) -> Result<(), CommandError> {
        self.state.count = 0;
        Ok(())
    }
    fn set_reminders(&mut self, enabled: bool) -> Result<(), CommandError> {
        self.state.reminders = enabled;
        Ok(())
    }
    fn set_level(&mut self, value: f64) -> Result<(), CommandError> {
        self.state.level = value.clamp(0.0, 100.0);
        Ok(())
    }
    fn set_goal(&mut self, goal: Goal) -> Result<(), CommandError> {
        self.state.goal = goal;
        Ok(())
    }
}

impl watch_demo::Engine for WatchEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
