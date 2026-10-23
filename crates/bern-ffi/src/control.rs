//! Nonblocking JSON entry into the engine owned by a pure browser app.
//! Authentication and connection lifetime belong to the local relay; the
//! catalog, validation, execution and returned state stay in this runtime.
use crate::{BernApp, slice, with_app};
use bern_adapter_json::{
    ClientMessage, ServerMessage, SessionInfo, params_from_json, state_json, to_json,
};
use bern_command::{CallContext, Entry};
use bern_media::{MediaCommand, media_catalog};
use bern_protocol::{
    Cancellation, CommandError, Invocation, PROTOCOL_VERSION, SessionCommand, session_catalog,
};
use bern_runtime::SessionHandler;
use serde_json::Value as Json;

fn failure(request_id: Option<String>, code: &str, message: impl Into<String>) -> ServerMessage {
    ServerMessage::Error {
        request_id,
        error: CommandError::new(code, message),
    }
}

fn outcome(id: String, revision: u64, result: Result<Json, CommandError>) -> ServerMessage {
    ServerMessage::Result {
        request_id: id,
        status: "completed".into(),
        revision: revision.to_string(),
        ok: result.is_ok(),
        error: result.as_ref().err().cloned(),
        result: result.ok(),
    }
}

/// Accept one control envelope. JSON reply bytes arrive through `on_reply`
/// with the supplied token, after the real command has completed.
///
/// # Safety
/// `app` must be live; `bytes` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bern_app_control(
    app: *mut BernApp,
    bytes: *const u8,
    len: usize,
    token: u64,
) {
    let parsed = serde_json::from_slice::<Json>(unsafe { slice(bytes, len) });
    with_app(app, (), |app| {
        let callbacks = app.callbacks;
        let send = move |message: ServerMessage| {
            if let Ok(bytes) = serde_json::to_vec(&message) {
                callbacks.reply(token, &bytes);
            }
        };
        let envelope = match parsed {
            Ok(value) => value,
            Err(error) => {
                send(failure(None, "invalid_message", error.to_string()));
                return;
            }
        };
        let entry = match envelope["entry"].as_str() {
            Some("cli") => Entry::Cli,
            Some("mcp") => Entry::Mcp,
            _ => Entry::Json,
        };
        let request_id = envelope["message"]["request_id"]
            .as_str()
            .map(str::to_owned);
        let message = match serde_json::from_value::<ClientMessage>(envelope["message"].clone()) {
            Ok(message) => message,
            Err(error) => {
                send(failure(request_id, "invalid_message", error.to_string()));
                return;
            }
        };
        let descriptor = app.host.descriptor();
        let revision = app.host.snapshot().0;
        match message {
            ClientMessage::Hello {
                protocol_version,
                catalog_fingerprint,
                ..
            } => {
                if protocol_version != PROTOCOL_VERSION {
                    send(failure(
                        None,
                        "protocol_mismatch",
                        "control protocol version differs",
                    ));
                } else if catalog_fingerprint
                    .is_some_and(|f| f != descriptor.fingerprint().to_string())
                {
                    send(failure(
                        None,
                        "catalog_mismatch",
                        "rebuild the tools for this page's command catalog",
                    ));
                } else {
                    send(bern_adapter_json::welcome(
                        &app.host.engine_id(),
                        descriptor,
                    ));
                }
            }
            ClientMessage::Command {
                request_id,
                engine_id,
                command,
                params,
            } => {
                if engine_id != app.host.engine_id() {
                    send(failure(
                        Some(request_id),
                        "engine_mismatch",
                        "request names another browser engine",
                    ));
                } else if let Some((index, spec)) = descriptor.command(&command) {
                    match params_from_json(spec, &params, &descriptor.types) {
                        Ok(params) => app.host.dispatch(
                            Invocation {
                                command: index,
                                params,
                            },
                            CallContext {
                                entry,
                                request_id: request_id.clone(),
                            },
                            Cancellation::new(),
                            Box::new(move |result| {
                                send(outcome(
                                    request_id,
                                    result.revision,
                                    result
                                        .result
                                        .map(|v| to_json(&v, &spec.result, &descriptor.types)),
                                ))
                            }),
                        ),
                        Err(error) => send(outcome(request_id, revision, Err(error))),
                    }
                } else if let Some(command) = MediaCommand::from_id(&command) {
                    let (types, specs) = media_catalog();
                    let spec = specs[command.index() as usize].clone();
                    match params_from_json(&spec, &params, &types) {
                        Ok(params) => app.media.dispatch(
                            command,
                            params,
                            Box::new(move |result| {
                                send(outcome(
                                    request_id,
                                    revision,
                                    result.map(|v| to_json(&v, &spec.result, &types)),
                                ))
                            }),
                        ),
                        Err(error) => send(outcome(request_id, revision, Err(error))),
                    }
                } else {
                    send(outcome(
                        request_id,
                        revision,
                        Err(CommandError::unknown_command(&command)),
                    ));
                }
            }
            ClientMessage::State { request_id } => {
                let (revision, values) = app.host.snapshot();
                send(ServerMessage::State {
                    request_id,
                    revision: revision.to_string(),
                    values: state_json(
                        descriptor,
                        &values.into_iter().enumerate().collect::<Vec<_>>(),
                    ),
                });
            }
            ClientMessage::Catalog { request_id } => {
                let (session_types, session_commands) = session_catalog();
                let (media_types, media_commands) = media_catalog();
                send(ServerMessage::Catalog {
                    request_id,
                    protocol: Box::new(descriptor.clone()),
                    session_types,
                    session_commands,
                    media_types,
                    media_commands,
                });
            }
            ClientMessage::Sessions { request_id } => send(ServerMessage::Sessions {
                request_id,
                sessions: vec![SessionInfo {
                    session_id: app.bridge.session_id(),
                    windows: app.bridge.windows(),
                }],
            }),
            ClientMessage::SessionCommand {
                request_id,
                session_id,
                command,
                params,
            } => {
                if session_id.is_some_and(|id| id != app.bridge.session_id()) {
                    send(failure(
                        Some(request_id),
                        "session_not_found",
                        "session is not in this browser page",
                    ));
                    return;
                }
                let (types, specs) = session_catalog();
                match (
                    SessionCommand::from_id(&command),
                    specs.into_iter().find(|s| s.id == command),
                ) {
                    (Some(command), Some(spec)) => match params_from_json(&spec, &params, &types) {
                        Ok(params) => {
                            app.bridge.run(
                                command,
                                params,
                                Box::new(move |result| {
                                    send(outcome(
                                        request_id,
                                        revision,
                                        result.map(|v| to_json(&v, &spec.result, &types)),
                                    ))
                                }),
                            );
                        }
                        Err(error) => send(outcome(request_id, revision, Err(error))),
                    },
                    _ => send(outcome(
                        request_id,
                        revision,
                        Err(CommandError::unknown_command(&command)),
                    )),
                }
            }
            _ => send(failure(
                request_id,
                "unsupported_message",
                "the browser bridge supports commands, queries, state, catalog and sessions; streaming subscriptions are not available",
            )),
        }
    });
}
