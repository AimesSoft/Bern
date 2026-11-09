//! One physical cache path, even when a worktree's target is a symlink.

use std::path::{Path, PathBuf};

pub fn module_cache(root: &Path) -> PathBuf {
    let cache = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"))
        .join("bern-swift-module-cache");
    std::fs::create_dir_all(&cache).unwrap();
    cache.canonicalize().unwrap()
}
