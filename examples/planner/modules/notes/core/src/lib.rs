//! Engine of the notes module. It knows nothing about the planner shell
//! or the other modules.

use bern::CommandError;
use notes_protocol::notes::{self, Note, State};

pub struct NotesEngine {
    state: State,
    next_id: i64,
}

impl Default for NotesEngine {
    fn default() -> Self {
        Self {
            state: State {
                items: Vec::new(),
                count: 0,
            },
            next_id: 0,
        }
    }
}

impl notes::Commands for NotesEngine {
    fn add(&mut self, text: String) -> Result<i64, CommandError> {
        if text.trim().is_empty() {
            return Err(CommandError::new("empty_note", "a note needs some text"));
        }
        self.next_id += 1;
        self.state.items.push(Note {
            id: self.next_id,
            text: text.clone(),
        });
        notes::emit::added(self.next_id, text);
        self.state.count = self.state.items.len() as i64;
        Ok(self.next_id)
    }

    fn clear(&mut self) -> Result<(), CommandError> {
        self.state.items.clear();
        self.state.count = 0;
        Ok(())
    }
}

impl notes::Engine for NotesEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
