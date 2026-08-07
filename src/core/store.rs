//! A runtime collection of layouts, organized by device folders.
//!
//! Layouts are loaded from a device-specific folder plus a shared `common`
//! folder. The device folder wins on name conflicts, so an app ships one set
//! of shared building blocks and per-device overrides.

use crate::core::layout::Layout;
use std::collections::HashMap;
use std::path::Path;

/// Loaded layouts, resolvable by name (the `.ron` file stem).
#[derive(Debug, Default)]
pub struct LayoutStore {
    layouts: HashMap<String, Layout>,
}

impl LayoutStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads every `*.ron` in `common_dir`, then every `*.ron` in
    /// `device_dir`. On name conflicts the device version wins.
    pub fn load(
        device_dir: impl AsRef<Path>,
        common_dir: impl AsRef<Path>,
    ) -> Result<Self, String> {
        let mut store = Self::new();
        store.load_dir(common_dir.as_ref())?;
        store.load_dir(device_dir.as_ref())?;
        Ok(store)
    }

    /// Inserts a layout under `name` (used by tests and programmatic loading).
    pub fn insert(&mut self, name: impl Into<String>, layout: Layout) {
        self.layouts.insert(name.into(), layout);
    }

    /// Resolves a layout by name.
    pub fn resolve(&self, name: &str) -> Option<&Layout> {
        self.layouts.get(name)
    }

    /// All layout names currently in the store.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.layouts.keys()
    }

    fn load_dir(&mut self, dir: &Path) -> Result<(), String> {
        let entries = std::fs::read_dir(dir)
            .map_err(|e| format!("read layouts dir `{}`: {e}", dir.display()))?;

        let mut files: Vec<_> = entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        files.sort();

        for path in files {
            let layout = Layout::load(&path)?;
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| format!("bad layout file name: {}", path.display()))?
                .to_string();
            self.layouts.insert(name, layout);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_common_and_device_layouts() {
        let store = LayoutStore::load("layouts/desktop", "layouts/common").expect("store loads");

        assert!(
            store.resolve("login_form").is_some(),
            "common block missing"
        );
        assert!(
            store.resolve("login_page").is_some(),
            "desktop page missing"
        );
    }
}
