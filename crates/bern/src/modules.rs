//! Commands for optional modules, queued by the shell engine.

/// Switch a module after the current command completes. Dependencies are
/// resolved by the composite engine, just as for `modules.set_enabled`.
pub fn set_enabled(id: &str, enabled: bool) {
    bern_command::outbox::call(
        "modules",
        0,
        vec![
            bern_protocol::Value::Text(id.into()),
            bern_protocol::Value::Bool(enabled),
        ],
    );
}
