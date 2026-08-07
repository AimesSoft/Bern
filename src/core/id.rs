//! Central registry of widget ids.
//!
//! The application declares every id in one place (typically a single
//! `ids.rs`), registers them here, and the layout runtime rejects layouts
//! that reference interactive widgets whose id was not declared. This keeps
//! id management in one file while catching typos and drift at load time.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// The set of widget ids declared by the application.
#[derive(Debug, Default, Clone)]
pub struct IdRegistry(Arc<Mutex<HashSet<String>>>);

impl IdRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares one id.
    pub fn register(&self, id: impl Into<String>) {
        if let Ok(mut ids) = self.0.lock() {
            ids.insert(id.into());
        }
    }

    /// Declares many ids (e.g. `ids::ALL` from the central `ids.rs`).
    pub fn register_all(&self, ids: impl IntoIterator<Item = impl AsRef<str>>) {
        if let Ok(mut registered) = self.0.lock() {
            registered.extend(ids.into_iter().map(|id| id.as_ref().to_string()));
        }
    }

    /// Whether `id` has been declared.
    pub fn contains(&self, id: &str) -> bool {
        self.0.lock().map(|ids| ids.contains(id)).unwrap_or(false)
    }

    /// Number of declared ids.
    pub fn len(&self) -> usize {
        self.0.lock().map(|ids| ids.len()).unwrap_or(0)
    }

    /// Whether no ids have been declared yet.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_and_checks_ids() {
        let registry = IdRegistry::new();
        assert!(registry.is_empty());
        registry.register_all(["a", "b"]);
        assert_eq!(registry.len(), 2);
        assert!(registry.contains("a"));
        assert!(!registry.contains("c"));
    }
}
