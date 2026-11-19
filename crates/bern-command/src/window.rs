//! Open, close, and inspect declared application windows from the engine.

use std::sync::{Arc, RwLock};

/// Bridge to the active UI session manager (installed by `bern-ffi`).
pub trait Host: Send + Sync {
    fn open(&self, id: &str);
    fn close(&self, id: &str);
    fn is_open(&self, id: &str) -> bool;
}

static HOST: RwLock<Option<Arc<dyn Host>>> = RwLock::new(None);

/// Install the host backing [`open`], [`close`], and [`is_open`].
pub fn install(host: impl Host + 'static) {
    *HOST.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(host));
}

fn host() -> Option<Arc<dyn Host>> {
    HOST.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Request that the UI open the declared window `id`, or bring it forward.
pub fn open(id: &str) {
    if let Some(host) = host() {
        host.open(id);
    }
}

/// Request that the UI close the declared window `id`.
pub fn close(id: &str) {
    if let Some(host) = host() {
        host.close(id);
    }
}

/// Whether a window of kind `id` is currently open.
pub fn is_open(id: &str) -> bool {
    host().is_some_and(|host| host.is_open(id))
}
