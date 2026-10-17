//! Small application-owned persistent values, isolated by the host bundle ID.
//! Hosts install a store before constructing the engine. Headless engines
//! without a host use an ephemeral store; tests can inject a directory.
use bern_protocol::CommandError;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Default)]
pub struct Store {
    root: Option<PathBuf>,
}
static HOST: OnceLock<Store> = OnceLock::new();
#[cfg(not(target_arch = "wasm32"))]
static SERIAL: AtomicU64 = AtomicU64::new(0);
impl Store {
    pub fn host() -> Self {
        HOST.get().cloned().unwrap_or_default()
    }
    pub fn directory(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
        }
    }
    pub fn install_application(bundle_id: &str) {
        if !valid_name(bundle_id) {
            return;
        }
        let base = if cfg!(target_arch = "wasm32") {
            // The browser host interprets this as an application namespace,
            // not a filesystem path. It stays isolated per web origin.
            Some(PathBuf::new())
        } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
        } else if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "linux") {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        } else {
            None
        };
        let _ = HOST.set(Self {
            root: base.map(|p| p.join(bundle_id)),
        });
    }
    fn path(&self, key: &str) -> Result<Option<PathBuf>, CommandError> {
        if !valid_name(key) {
            return Err(CommandError::new(
                "invalid_storage_key",
                "storage keys must be simple names",
            ));
        }
        Ok(self.root.as_ref().map(|p| p.join(key)))
    }
    pub fn read(&self, key: &str) -> Result<Option<Vec<u8>>, CommandError> {
        let Some(path) = self.path(key)? else {
            return Ok(None);
        };
        #[cfg(target_arch = "wasm32")]
        return browser::read(&path);
        #[cfg(not(target_arch = "wasm32"))]
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CommandError::new("storage_read", e.to_string())),
        }
    }
    pub fn write(&self, key: &str, bytes: &[u8]) -> Result<(), CommandError> {
        let Some(path) = self.path(key)? else {
            return Ok(());
        };
        #[cfg(target_arch = "wasm32")]
        return browser::write(&path, bytes);
        #[cfg(not(target_arch = "wasm32"))]
        write_atomic(&path, bytes).map_err(|e| CommandError::new("storage_write", e.to_string()))
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}
#[cfg(not(target_arch = "wasm32"))]
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path.parent().expect("storage parent");
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".bern-{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;

    #[link(wasm_import_module = "bern")]
    unsafe extern "C" {
        fn storage_read(
            app: *const u8,
            app_len: usize,
            key: *const u8,
            key_len: usize,
            out: *mut u8,
            capacity: usize,
        ) -> i32;
        fn storage_write(
            app: *const u8,
            app_len: usize,
            key: *const u8,
            key_len: usize,
            bytes: *const u8,
            len: usize,
        ) -> i32;
    }

    fn names(path: &Path) -> (&str, &str) {
        (
            path.parent().and_then(Path::to_str).unwrap_or_default(),
            path.file_name()
                .and_then(|v| v.to_str())
                .unwrap_or_default(),
        )
    }

    pub fn read(path: &Path) -> Result<Option<Vec<u8>>, CommandError> {
        let (app, key) = names(path);
        // SAFETY: input strings are live, and a zero capacity requests size only.
        let len = unsafe {
            storage_read(
                app.as_ptr(),
                app.len(),
                key.as_ptr(),
                key.len(),
                std::ptr::null_mut(),
                0,
            )
        };
        if len == -1 {
            return Ok(None);
        }
        if len < 0 {
            return Err(CommandError::new(
                "storage_read",
                "Browser storage is unavailable or the saved value is invalid",
            ));
        }
        let mut bytes = vec![0; len as usize];
        // SAFETY: output buffer is writable for its declared capacity.
        let actual = unsafe {
            storage_read(
                app.as_ptr(),
                app.len(),
                key.as_ptr(),
                key.len(),
                bytes.as_mut_ptr(),
                bytes.len(),
            )
        };
        if actual < -1 {
            return Err(CommandError::new(
                "storage_read",
                "Browser storage is unavailable or the saved value is invalid",
            ));
        }
        if actual != len {
            return Err(CommandError::new(
                "storage_read",
                "The saved browser value changed while reading",
            ));
        }
        Ok(Some(bytes))
    }

    pub fn write(path: &Path, bytes: &[u8]) -> Result<(), CommandError> {
        let (app, key) = names(path);
        // SAFETY: all slices remain live throughout the synchronous host call.
        let status = unsafe {
            storage_write(
                app.as_ptr(),
                app.len(),
                key.as_ptr(),
                key.len(),
                bytes.as_ptr(),
                bytes.len(),
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(CommandError::new(
                "storage_write",
                "Browser storage is unavailable or full",
            ))
        }
    }
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn store_replaces_values_and_rejects_path_traversal() {
        let root = std::env::temp_dir().join(format!(
            "bern-store-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let store = Store::directory(&root);
        assert!(store.read("first-run").unwrap().is_none());
        store.write("first-run", b"one").unwrap();
        store.write("first-run", b"two").unwrap();
        assert_eq!(store.read("first-run").unwrap(), Some(b"two".to_vec()));
        assert!(store.write("../escape", b"x").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
