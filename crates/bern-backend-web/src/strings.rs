//! Per-language string tables for the page, as the JSON the runtime reads.

use bern_ir::Application;
use serde_json::{Map, Value};

/// `{ language: { key: text } }` for every supported language, from the
/// tables every runtime that fills `{name}` placeholders by name shares.
pub fn tables(app: &Application) -> Value {
    let tables: Map<String, Value> = bern_target_api::texts::tables(app)
        .into_iter()
        .map(|(language, table)| {
            let table: Map<String, Value> = table
                .into_iter()
                .map(|(key, text)| (key, Value::String(text)))
                .collect();
            (language, Value::Object(table))
        })
        .collect();
    Value::Object(tables)
}
