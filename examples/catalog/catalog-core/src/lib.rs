//! Engine of the catalog.

use bern::CommandError;
use catalog_protocol::catalog::{self, Entry, Line, State};

pub struct CatalogEngine {
    state: State,
    next: i64,
    renamed: i64,
}

/// Words the rows are named from.
const WORDS: [&str; 12] = [
    "Amber", "Birch", "Cobalt", "Dune", "Ember", "Fjord", "Garnet", "Harbor", "Indigo", "Juniper",
    "Kelp", "Lagoon",
];

fn entry(id: i64) -> Entry {
    let word = WORDS[(id as usize) % WORDS.len()];
    let other = WORDS[(id as usize * 7 + 3) % WORDS.len()];
    Entry {
        id,
        title: format!("{word} {other} {id}"),
        // Cards of one row have details of different lengths.
        detail: std::iter::repeat_n(format!("{other} {word}"), 1 + (id as usize % 5))
            .collect::<Vec<_>>()
            .join(" · "),
        number: 0,
    }
}

impl Default for CatalogEngine {
    fn default() -> Self {
        let mut engine = Self {
            state: State {
                entries: (1..=5000).map(entry).collect(),
                count: 0,
                // Keys of their own: the scroll view finds items by key.
                cards: (100_001..=100_600).map(entry).collect(),
                opened: String::new(),
                lines: (1..=60)
                    .map(|key| Line {
                        key,
                        text: format!(
                            "{} {} {}",
                            WORDS[key as usize % WORDS.len()],
                            WORDS[(key as usize * 5 + 1) % WORDS.len()],
                            key
                        ),
                        current: false,
                        blur: 0.0,
                    })
                    .collect(),
                scroll_to: 1,
                scroll_tick: 0,
                browsing: false,
            },
            next: 5001,
            renamed: 0,
        };
        engine.number();
        engine.light(1);
        engine
    }
}

impl CatalogEngine {
    fn number(&mut self) {
        for (place, entry) in self.state.entries.iter_mut().enumerate() {
            entry.number = place as i64 + 1;
        }
        self.state.count = self.state.entries.len() as i64;
    }

    /// Line `current` is the current one, the others blurred by distance.
    fn light(&mut self, current: i64) {
        let browsing = self.state.browsing;
        for line in &mut self.state.lines {
            let away = (line.key - current).abs() as f64;
            line.current = line.key == current;
            line.blur = if browsing { 0.0 } else { (away * 1.5).min(6.0) };
        }
    }

    fn current(&self) -> i64 {
        self.state
            .lines
            .iter()
            .find(|line| line.current)
            .map_or(1, |line| line.key)
    }
}

impl catalog::Commands for CatalogEngine {
    fn grow(&mut self) -> Result<(), CommandError> {
        let start = self.next;
        self.next += 1000;
        self.state.entries.extend((start..self.next).map(entry));
        self.number();
        Ok(())
    }

    fn rename(&mut self) -> Result<(), CommandError> {
        self.renamed += 1;
        for entry in self.state.entries.iter_mut().take(10) {
            entry.title = format!(
                "{} ✎{}",
                entry.title.split(" ✎").next().unwrap_or(""),
                self.renamed
            );
        }
        Ok(())
    }

    fn trim(&mut self) -> Result<(), CommandError> {
        let keep = self.state.entries.len().saturating_sub(10);
        self.state.entries.drain(..self.state.entries.len() - keep);
        self.number();
        Ok(())
    }

    fn open(&mut self, id: i64) -> Result<(), CommandError> {
        if let Some(entry) = self.state.entries.iter().find(|entry| entry.id == id) {
            self.state.opened = entry.title.clone();
        }
        Ok(())
    }

    fn remove(&mut self, id: i64) -> Result<(), CommandError> {
        self.state.entries.retain(|entry| entry.id != id);
        self.number();
        Ok(())
    }

    fn step(&mut self) -> Result<(), CommandError> {
        let next = self.current() % self.state.lines.len() as i64 + 1;
        self.light(next);
        if !self.state.browsing {
            self.follow();
        }
        Ok(())
    }

    fn browse(&mut self, active: bool) -> Result<(), CommandError> {
        self.state.browsing = active;
        self.light(self.current());
        // Back to the current line once the person lets go.
        if !active {
            self.follow();
        }
        Ok(())
    }
}

impl CatalogEngine {
    fn follow(&mut self) {
        self.state.scroll_to = self.current();
        self.state.scroll_tick += 1;
    }
}

impl catalog::Engine for CatalogEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
