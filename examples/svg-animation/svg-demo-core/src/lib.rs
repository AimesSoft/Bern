use bern::CommandError;
use svg_demo_protocol::svg_demo::{self, State};
pub struct SvgEngine {
    state: State,
}
impl Default for SvgEngine {
    fn default() -> Self {
        Self {
            state: State {
                playing: true,
                speed: 1.0,
                revision: 0,
            },
        }
    }
}
impl svg_demo::Commands for SvgEngine {
    fn toggle(&mut self) -> Result<(), CommandError> {
        self.state.playing = !self.state.playing;
        Ok(())
    }
    fn faster(&mut self) -> Result<(), CommandError> {
        self.state.speed = if self.state.speed == 1.0 { 2.0 } else { 1.0 };
        Ok(())
    }
    fn restart(&mut self) -> Result<(), CommandError> {
        self.state.revision += 1;
        Ok(())
    }
}
impl svg_demo::Engine for SvgEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
