//! Small host services that need the platform's document and UI APIs.
//!
//! Android installs these services before starting the engine. Calls are
//! synchronous and may copy large files; engines should perform imports
//! and exports on a worker thread. Other hosts report an explicit
//! `platform_unavailable` error until they install an implementation.

use bern_protocol::CommandError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

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

pub(crate) fn request(operation: &str, first: &str, second: &str) -> Result<String, CommandError> {
    // In the browser the page is the host: the engine reaches its
    // clipboard through the page's imports.
    #[cfg(target_arch = "wasm32")]
    if operation == "copy_text" && SERVICES.read().unwrap_or_else(|e| e.into_inner()).is_none() {
        #[link(wasm_import_module = "bern")]
        unsafe extern "C" {
            fn copy_text(ptr: *const u8, len: usize) -> i32;
        }
        // SAFETY: the page reads `len` bytes at `ptr` before returning.
        let status = unsafe { copy_text(first.as_ptr(), first.len()) };
        return if status == 0 {
            Ok(String::new())
        } else {
            Err(CommandError::new(
                "platform_unavailable",
                "the page has no clipboard",
            ))
        };
    }
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

/// Save the picture at `path`, a PNG or a JPEG, where the system keeps the
/// user's photos, for other apps to pick from: the photo library on iOS,
/// which asks the user the first time (`[app].photos` says why), and the
/// Pictures collection on Android 10 and later. `refused` when the user
/// does not allow it, `platform_unavailable` where there is no such place.
/// The call waits for the system's answer: make it on a worker thread.
pub fn save_image(path: impl AsRef<Path>) -> Result<(), CommandError> {
    request("save_image", path_text(path.as_ref())?, "").map(|_| ())
}

/// Open a local file or a `content://` document with another application.
/// Android grants read access only to that file. Private directories must
/// be exported with [`export_path`] before another application can browse them.
pub fn open_path(path: impl AsRef<Path>) -> Result<(), CommandError> {
    request("open_path", path_text(path.as_ref())?, "").map(|_| ())
}

/// Whether the user lets the application reach every file on shared
/// storage by path, with `std::fs`: `false` on Android until the user allows
/// it, `true` everywhere else, where there is nothing to ask.
pub fn all_files_allowed() -> bool {
    #[cfg(target_os = "android")]
    return request("all_files_allowed", "", "").is_ok_and(|answer| answer == "yes");
    #[cfg(not(target_os = "android"))]
    true
}

/// The engine that asked last, to wake when the user has answered.
static ASKED: Mutex<Option<crate::wake::Waker>> = Mutex::new(None);

/// Let the user allow reaching every file on shared storage (Android, whose
/// target needs `android = { all_files_access = true }`); nothing elsewhere.
/// Android 11 and later open the application's page in the system's
/// settings, and earlier versions show the storage permissions. Ask from a
/// command: when the user comes back the engine's `wake` runs, whatever the
/// answer, and reads [`all_files_allowed`] again. `platform_failed` when the
/// target does not declare it or the system has no such page.
pub fn ask_all_files() -> Result<(), CommandError> {
    *ASKED.lock().unwrap_or_else(|e| e.into_inner()) = crate::wake::waker();
    #[cfg(target_os = "android")]
    return request("all_files_ask", "", "").map(|_| ());
    #[cfg(not(target_os = "android"))]
    Ok(())
}

/// The user came back from what [`ask_all_files`] showed: what the host calls.
pub fn all_files_answered() {
    let asked = ASKED.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(waker) = asked {
        waker.wake();
    }
}

/// Make the application what opens the files `[[app.opens]]` declares, as
/// far as the system lets an application do so. macOS and Linux change it at
/// once, and Windows, which leaves the choice to the user, opens the
/// application's page in Settings. On Android and iOS the user chooses when
/// opening a file, so the answer is `platform_unavailable`.
pub fn make_default_opener() -> Result<(), CommandError> {
    request("make_default_opener", "", "").map(|_| ())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wake::{Waker, enter};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn the_answer_wakes_the_engine_that_asked_once() {
        let woken = Arc::new(AtomicUsize::new(0));
        let count = woken.clone();
        let waker = Waker::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            true
        });
        {
            let _running = enter(&waker);
            ask_all_files().unwrap();
        }
        all_files_answered();
        waker.taken();
        all_files_answered();
        assert_eq!(woken.load(Ordering::SeqCst), 1);
    }
}
