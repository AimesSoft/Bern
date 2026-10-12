//! Engine of the tags module.

use bern::CommandError;
use notes_protocol::notes;
use tags_protocol::tags::{self, State, Tag};

pub struct TagsEngine {
    state: State,
}

impl Default for TagsEngine {
    fn default() -> Self {
        Self {
            state: State { labels: Vec::new() },
        }
    }
}

impl tags::Commands for TagsEngine {
    fn tag(&mut self, label: String) -> Result<(), CommandError> {
        self.add(label);
        Ok(())
    }

    fn label_note(&mut self, _id: i64, text: String) -> Result<(), CommandError> {
        for word in text.split_whitespace() {
            if let Some(label) = word.strip_prefix('#').filter(|label| !label.is_empty()) {
                self.add(label.to_string());
            }
        }
        Ok(())
    }

    fn note_count(&mut self) -> Result<(), CommandError> {
        let count = notes::current_state().map_or(0, |notes| notes.count);
        notes::calls::add(format!("{count} notes so far"));
        Ok(())
    }
}

impl TagsEngine {
    fn add(&mut self, label: String) {
        if !self.state.labels.iter().any(|tag| tag.name == label) {
            self.state.labels.push(Tag { name: label });
        }
    }
}

impl tags::Engine for TagsEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
