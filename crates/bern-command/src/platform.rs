//! Small host services that need the platform's document and UI APIs.
//!
//! Android installs these services before starting the engine. Calls are
//! synchronous and may copy large files; engines should perform imports
//! and exports on a worker thread. Other hosts report an explicit
//! `platform_unavailable` error until they install an implementation.

use bern_protocol::CommandError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// The host implementation, installed before commands are accepted.
pub trait Services: Send + Sync {
    /// Execute one of the operations exposed by this module.
    fn request(&self, operation: &str, first: &str, second: &str) -> Result<String, CommandError>;
}

static SERVICES: RwLock<Option<Arc<dyn Services>>> = RwLock::new(None);

/// Install platform services. Hosts call this before creating the engine.
pub fn install(services: impl Services + 'static) {
    *SERVICES.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(services));
}

fn request(operation: &str, first: &str, second: &str) -> Result<String, CommandError> {
    let host = SERVICES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or_else(|| {
            CommandError::new("platform_unavailable", "the host has no platform services")
        })?;
    host.request(operation, first, second)
}

fn path_text(path: &Path) -> Result<&str, CommandError> {
    path.to_str()
        .ok_or_else(|| CommandError::new("invalid_path", "the path is not valid UTF-8"))
}

/// Copy text using the system clipboard on the UI thread.
pub fn copy_text(text: &str) -> Result<(), CommandError> {
    request("copy_text", text, "").map(|_| ())
}

/// Open a local file or a `content://` document with another application.
/// Android grants read access only to that file. Private directories must
/// be exported with [`export_path`] before another application can browse them.
pub fn open_path(path: impl AsRef<Path>) -> Result<(), CommandError> {
    request("open_path", path_text(path.as_ref())?, "").map(|_| ())
}

/// Recursively import a user-selected Android document tree into a local
/// directory. Returns the new root, named after the selected folder.
/// Existing local files are never overwritten; colliding names are numbered.
pub fn import_folder(uri: &str, destination: impl AsRef<Path>) -> Result<PathBuf, CommandError> {
    request("import_folder", uri, path_text(destination.as_ref())?).map(PathBuf::from)
}

/// Export a local file or directory into a user-selected Android document
/// tree. Returns the created document URI; colliding names are numbered.
/// The source is retained, and the selected tree must have write permission.
pub fn export_path(source: impl AsRef<Path>, tree_uri: &str) -> Result<String, CommandError> {
    request("export_path", path_text(source.as_ref())?, tree_uri)
}
