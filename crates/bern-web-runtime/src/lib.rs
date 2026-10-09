//! Browser-local session commands and the shared Rust theme calculation.
//!
//! This crate has no application engine implementation. JavaScript only adapts
//! the DOM, connects the remote engine, and calls this typed session boundary.

mod session;
mod theme;

use session::Session;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WebSession {
    inner: Session,
}

#[wasm_bindgen]
impl WebSession {
    #[wasm_bindgen(constructor)]
    pub fn new(ui_json: &str, session_id: &str) -> Result<WebSession, JsValue> {
        Session::from_json(ui_json, session_id)
            .map(|inner| Self { inner })
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Target is the stable component ID from the generated UI, never a DOM
    /// object or an arbitrary field name. Failed commands do not change state.
    pub fn dispatch(&mut self, command: &str, target: &str, payload_json: &str) -> String {
        self.inner.dispatch_json(command, target, payload_json)
    }

    pub fn snapshot(&self) -> String {
        self.inner.snapshot_json()
    }

    pub fn catalog(&self) -> String {
        self.inner.catalog_json()
    }

    /// Returns CSS-ready semantic tokens calculated entirely in Rust.
    pub fn theme(&self, mode: &str) -> Result<String, JsValue> {
        theme::generate(mode)
            .and_then(|theme| serde_json::to_string(&theme).map_err(|error| error.to_string()))
            .map_err(|error| JsValue::from_str(&error))
    }
}
