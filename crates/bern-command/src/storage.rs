//! Small application-owned persistent values, isolated by the host bundle ID.
//! Hosts install a store before constructing the engine. Headless engines
//! without a host use an ephemeral store; tests can inject a directory.
//!
//! The same installation tells the application where its larger files go:
//! [`data_dir`] for its own data that is not a setting, such as a database,
//! and [`cache_dir`] for files it can fetch again.
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
/// The application's identity on this device, and its folders.
static APPLICATION: OnceLock<(String, Folders)> = OnceLock::new();
/// Android has no home folder; its host passes the application's private
/// files folder before installing the application.
static ANDROID_FILES: OnceLock<PathBuf> = OnceLock::new();
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
        let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let folders = Folders::of(
            std::env::consts::OS,
            &env,
            ANDROID_FILES.get().map(PathBuf::as_path),
            bundle_id,
        );
        let settings = if cfg!(target_arch = "wasm32") {
            // The browser host interprets this as an application namespace,
            // not a filesystem path. It stays isolated per web origin.
            Some(PathBuf::from(bundle_id))
        } else {
            folders.settings.clone()
        };
        let _ = HOST.set(Self { root: settings });
        let _ = APPLICATION.set((bundle_id.into(), folders));
    }

    /// Android's host passes the application's private files folder
    /// (`Context.getFilesDir()`) before installing the application.
    pub fn install_files_folder(path: impl Into<PathBuf>) {
        let _ = ANDROID_FILES.set(path.into());
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
/// The application's identity on this device, once a host installed it.
pub fn application_id() -> Option<&'static str> {
    APPLICATION.get().map(|(id, _)| id.as_str())
}

/// A folder for the application's own data that is not a setting, such as
/// a database; created when missing. `None` where the platform gives the
/// application no folders (the browser) or before a host installed it.
/// On tvOS this is inside the caches, which the system may empty when the
/// television runs short of room.
pub fn data_dir() -> Option<PathBuf> {
    created(APPLICATION.get()?.1.data.clone()?)
}

/// A folder for files the application can fetch again; created when
/// missing. The system or the user may empty it at any time.
pub fn cache_dir() -> Option<PathBuf> {
    created(APPLICATION.get()?.1.cache.clone()?)
}

fn created(path: PathBuf) -> Option<PathBuf> {
    std::fs::create_dir_all(&path).ok()?;
    Some(path)
}

/// Where an application keeps its things, each folder already named after
/// the application.
#[derive(Clone, Debug, Default, PartialEq)]
struct Folders {
    /// The small values of [`Store`].
    settings: Option<PathBuf>,
    data: Option<PathBuf>,
    cache: Option<PathBuf>,
}

impl Folders {
    /// The folders on `os`, the platform's own conventions: Application
    /// Support and Caches on Apple systems, the roaming and local
    /// application data on Windows, the XDG base directories on Linux.
    fn of(
        os: &str,
        env: &dyn Fn(&str) -> Option<PathBuf>,
        android_files: Option<&Path>,
        id: &str,
    ) -> Self {
        let home = || env("HOME");
        // An XDG variable counts only when it is an absolute path.
        let xdg = |name: &str, fallback: &str| {
            env(name)
                .filter(|path| path.is_absolute())
                .or_else(|| home().map(|home| home.join(fallback)))
        };
        let (settings, data, cache) = match os {
            "macos" | "ios" => {
                let support = home().map(|home| home.join("Library/Application Support"));
                let caches = home().map(|home| home.join("Library/Caches"));
                (support.clone(), support, caches)
            }
            // A tvOS app may write only its caches.
            "tvos" => {
                let caches = home().map(|home| home.join("Library/Caches"));
                (caches.clone(), caches.clone(), caches)
            }
            "windows" => {
                let local = env("LOCALAPPDATA");
                (env("APPDATA"), local.clone(), local)
            }
            "linux" => (
                xdg("XDG_CONFIG_HOME", ".config"),
                xdg("XDG_DATA_HOME", ".local/share"),
                xdg("XDG_CACHE_HOME", ".cache"),
            ),
            "android" => {
                let files = android_files.map(Path::to_path_buf);
                // `getCacheDir()` sits beside `getFilesDir()`.
                let cache = android_files
                    .and_then(Path::parent)
                    .map(|app| app.join("cache"));
                (files.clone(), files, cache)
            }
            _ => (None, None, None),
        };
        let named = |path: Option<PathBuf>| path.map(|path| path.join(id));
        let (settings, data, mut cache) = (named(settings), named(data), named(cache));
        // Where data and caches share one folder, the caches get their own.
        if cache.is_some() && cache == data {
            cache = cache.map(|cache| cache.join("Cache"));
        }
        Self {
            settings,
            data,
            cache,
        }
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

    #[test]
    fn each_platform_keeps_settings_data_and_caches_where_it_expects_them() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| PathBuf::from(value))
            }
        };
        let folders = |os, pairs, files: Option<&str>| {
            let f = Folders::of(os, &env(pairs), files.map(Path::new), "app.id");
            let show = |path: Option<PathBuf>| path.map(|path| path.display().to_string());
            (show(f.settings), show(f.data), show(f.cache))
        };
        let some = |path: &str| Some(path.to_string());
        assert_eq!(
            folders("macos", &[("HOME", "/Users/a")], None),
            (
                some("/Users/a/Library/Application Support/app.id"),
                some("/Users/a/Library/Application Support/app.id"),
                some("/Users/a/Library/Caches/app.id")
            )
        );
        assert_eq!(
            folders("tvos", &[("HOME", "/h")], None),
            (
                some("/h/Library/Caches/app.id"),
                some("/h/Library/Caches/app.id"),
                some("/h/Library/Caches/app.id/Cache")
            )
        );
        assert_eq!(
            folders(
                "windows",
                &[("APPDATA", "C:/R"), ("LOCALAPPDATA", "C:/L")],
                None
            ),
            (
                some("C:/R/app.id"),
                some("C:/L/app.id"),
                some("C:/L/app.id/Cache")
            )
        );
        assert_eq!(
            folders(
                "linux",
                &[("HOME", "/home/a"), ("XDG_CACHE_HOME", "relative")],
                None
            ),
            (
                some("/home/a/.config/app.id"),
                some("/home/a/.local/share/app.id"),
                some("/home/a/.cache/app.id")
            )
        );
        assert_eq!(
            folders(
                "linux",
                &[("HOME", "/home/a"), ("XDG_DATA_HOME", "/d")],
                None
            )
            .1,
            some("/d/app.id")
        );
        assert_eq!(
            folders("android", &[], Some("/data/user/0/pkg/files")),
            (
                some("/data/user/0/pkg/files/app.id"),
                some("/data/user/0/pkg/files/app.id"),
                some("/data/user/0/pkg/cache/app.id")
            )
        );
        assert_eq!(folders("android", &[], None), (None, None, None));
        assert_eq!(folders("macos", &[], None), (None, None, None));
    }
}
