//! MCP server derived from the command catalog.
//!
//! Speaks MCP over stdio (JSON-RPC 2.0, one message per line). Tools map to
//! engine commands and framework session commands with schemas derived from
//! the catalog; every call goes to the running engine through the local
//! control channel, the same host the UI and CLI use. Custom control-channel
//! envelopes are never passed off as MCP messages.

use bern_adapter_json::{ClientMessage, ServerMessage, command_input_schema};
use bern_protocol::{CommandSpec, OperationKind, ProtocolDescriptor, Retry, session_catalog};
use bern_transport_ipc::Client;
use serde_json::{Map, Value as Json, json};
use std::io::{BufRead, Write};

/// Protocol versions this server can speak, newest first.
pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

pub struct Server<'a> {
    descriptor: &'a ProtocolDescriptor,
    bundle_id: String,
    name: String,
    version: String,
    client: Option<Client>,
}

fn tool(spec: &CommandSpec, input: Json, session: bool) -> Json {
    let mut schema = input;
    if session {
        schema["properties"]["session_id"] = json!({
            "type": "string",
            "description": "Live UI session; optional when exactly one is running"
        });
    }
    let mut description = if spec.doc.is_empty() {
        spec.id.clone()
    } else {
        spec.doc.clone()
    };
    if session {
        description.push_str(" (acts on the running UI through its real controls)");
    }
    json!({
        "name": spec.external.mcp,
        "title": spec.id,
        "description": description,
        "inputSchema": schema,
        "annotations": {
            "readOnlyHint": spec.kind == OperationKind::Query,
            "idempotentHint": spec.policy.retry == Retry::Idempotent,
            "openWorldHint": false
        }
    })
}

fn text_result(text: String, is_error: bool) -> Json {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

impl<'a> Server<'a> {
    pub fn new(
        descriptor: &'a ProtocolDescriptor,
        bundle_id: &str,
        name: &str,
        version: &str,
    ) -> Self {
        Self {
            descriptor,
            bundle_id: bundle_id.into(),
            name: name.into(),
            version: version.into(),
            client: None,
        }
    }

    pub fn tools(&self) -> Vec<Json> {
        let mut tools: Vec<Json> = self
            .descriptor
            .commands
            .iter()
            .map(|spec| {
                tool(
                    spec,
                    command_input_schema(spec, &self.descriptor.types),
                    false,
                )
            })
            .collect();
        let (session_types, session_specs) = session_catalog();
        tools.extend(
            session_specs
                .iter()
                .map(|spec| tool(spec, command_input_schema(spec, &session_types), true)),
        );
        tools.push(json!({
            "name": "bern_state",
            "title": "engine state",
            "description": format!("Read the current state of the `{}` engine", self.descriptor.namespace),
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false }
        }));
        tools.push(json!({
            "name": "bern_sessions",
            "title": "UI sessions",
            "description": "List live UI sessions and their open windows",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
            "annotations": { "readOnlyHint": true, "idempotentHint": true, "openWorldHint": false }
        }));
        tools
    }

    fn connect(&mut self) -> Result<&mut Client, String> {
        if self.client.is_none() {
            let client = Client::connect(
                &self.bundle_id,
                None,
                "mcp",
                Some(self.descriptor.fingerprint()),
            )
            .map_err(|error| error.to_string())?;
            self.client = Some(client);
        }
        Ok(self.client.as_mut().unwrap())
    }

    fn call(&mut self, name: &str, mut arguments: Map<String, Json>) -> Json {
        let descriptor = self.descriptor;
        let (_, session_specs) = session_catalog();
        let message =
            if let Some(spec) = descriptor
                .commands
                .iter()
                .find(|spec| spec.external.mcp == name)
            {
                let id = spec.id.clone();
                Some(Box::new(
                    move |request_id: String, engine_id: String| ClientMessage::Command {
                        request_id,
                        engine_id,
                        command: id,
                        params: arguments,
                    },
                )
                    as Box<dyn FnOnce(String, String) -> ClientMessage>)
            } else if let Some(spec) = session_specs.iter().find(|spec| spec.external.mcp == name) {
                let id = spec.id.clone();
                let session_id = arguments
                    .remove("session_id")
                    .and_then(|value| value.as_str().map(str::to_string));
                Some(
                    Box::new(move |request_id: String, _| ClientMessage::SessionCommand {
                        request_id,
                        session_id,
                        command: id,
                        params: arguments,
                    }) as Box<dyn FnOnce(String, String) -> ClientMessage>,
                )
            } else {
                match name {
                    "bern_state" => {
                        Some(
                            Box::new(|request_id: String, _| ClientMessage::State { request_id })
                                as Box<dyn FnOnce(String, String) -> ClientMessage>,
                        )
                    }
                    "bern_sessions" => Some(Box::new(|request_id: String, _| {
                        ClientMessage::Sessions { request_id }
                    })
                        as Box<dyn FnOnce(String, String) -> ClientMessage>),
                    _ => None,
                }
            };
        let Some(message) = message else {
            return text_result(format!("unknown tool `{name}`"), true);
        };
        let client = match self.connect() {
            Ok(client) => client,
            Err(error) => return text_result(error, true),
        };
        let engine_id = client.engine_id.clone();
        match client.request(|request_id| message(request_id, engine_id)) {
            Ok(ServerMessage::Result {
                ok: true,
                result,
                revision,
                ..
            }) => text_result(
                serde_json::to_string(
                    &json!({ "result": result.unwrap_or(Json::Null), "revision": revision }),
                )
                .unwrap_or_default(),
                false,
            ),
            Ok(ServerMessage::Result {
                error: Some(error), ..
            })
            | Ok(ServerMessage::Error { error, .. }) => {
                text_result(format!("{}: {}", error.code, error.message), true)
            }
            Ok(ServerMessage::State {
                revision, values, ..
            }) => text_result(
                serde_json::to_string(&json!({ "revision": revision, "values": values }))
                    .unwrap_or_default(),
                false,
            ),
            Ok(ServerMessage::Sessions { sessions, .. }) => {
                text_result(serde_json::to_string(&sessions).unwrap_or_default(), false)
            }
            Ok(other) => text_result(format!("unexpected reply {other:?}"), true),
            Err(error) => {
                self.client = None;
                text_result(format!("connection lost: {error}"), true)
            }
        }
    }

    /// Handle one JSON-RPC message; returns the response, if any.
    pub fn handle(&mut self, message: &Json) -> Option<Json> {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Json::as_str).unwrap_or("");
        let params = message.get("params").cloned().unwrap_or(Json::Null);
        let result = match method {
            "initialize" => {
                let requested = params
                    .get("protocolVersion")
                    .and_then(Json::as_str)
                    .unwrap_or(PROTOCOL_VERSIONS[0]);
                let version = PROTOCOL_VERSIONS
                    .iter()
                    .find(|candidate| **candidate == requested)
                    .unwrap_or(&PROTOCOL_VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": self.name, "version": self.version },
                    "instructions": format!(
                        "Tools control the running `{}` application. Engine tools change business state; \
                         ui_* and window_* tools act on the live UI through its real controls — call ui_inspect \
                         first to find control targets.",
                        self.descriptor.namespace
                    )
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                let arguments = params
                    .get("arguments")
                    .and_then(Json::as_object)
                    .cloned()
                    .unwrap_or_default();
                Ok(self.call(&name, arguments))
            }
            method if method.starts_with("notifications/") => return None,
            _ => Err(json!({ "code": -32601, "message": format!("method `{method}` not found") })),
        };
        let id = id?;
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        })
    }
}

/// Serve MCP over stdio until stdin closes; returns the exit code.
pub fn run(descriptor: &ProtocolDescriptor, bundle_id: &str, name: &str, version: &str) -> i32 {
    let mut server = Server::new(descriptor, bundle_id, name, version);
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Json>(&line) {
            Ok(message) => server.handle(&message),
            Err(error) => Some(json!({
                "jsonrpc": "2.0",
                "id": Json::Null,
                "error": { "code": -32700, "message": error.to_string() }
            })),
        };
        if let Some(response) = response
            && (writeln!(stdout, "{response}").is_err() || stdout.flush().is_err())
        {
            break;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_protocol::{
        CommandPolicy, DESCRIPTOR_VERSION, ExternalNames, ParamSpec, Scope, StateField, Type,
        TypeTable,
    };

    fn descriptor() -> ProtocolDescriptor {
        ProtocolDescriptor {
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "player".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![StateField {
                name: "position_ms".into(),
                ty: Type::Int,
                doc: String::new(),
            }],
            commands: vec![CommandSpec {
                id: "player.seek".into(),
                name: "seek".into(),
                doc: "Move the playback position".into(),
                kind: OperationKind::Command,
                scope: Scope::Engine,
                params: vec![ParamSpec {
                    name: "position_ms".into(),
                    ty: Type::Int,
                    doc: String::new(),
                }],
                result: Type::Unit,
                errors: vec![],
                policy: CommandPolicy::default(),
                external: ExternalNames::derive("player", "seek"),
            }],
        }
    }

    #[test]
    fn initialize_and_list_tools_follow_mcp() {
        let descriptor = descriptor();
        let mut server = Server::new(&descriptor, "dev.bern.mcp-test", "player", "1.0.0");
        let reply = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } }))
            .unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
        assert!(
            server
                .handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
                .is_none()
        );
        let reply = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))
            .unwrap();
        let tools = reply["result"]["tools"].as_array().unwrap();
        let seek = tools
            .iter()
            .find(|tool| tool["name"] == "player_seek")
            .unwrap();
        assert_eq!(seek["inputSchema"]["required"], json!(["position_ms"]));
        let inspect = tools
            .iter()
            .find(|tool| tool["name"] == "ui_inspect")
            .unwrap();
        assert_eq!(inspect["annotations"]["readOnlyHint"], true);
        assert!(inspect["inputSchema"]["properties"]["session_id"].is_object());
    }

    #[test]
    fn calls_without_an_engine_are_tool_errors_not_protocol_errors() {
        let descriptor = descriptor();
        let mut server = Server::new(&descriptor, "dev.bern.mcp-test-none", "player", "1.0.0");
        let reply = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "player_seek", "arguments": { "position_ms": 5 } } }))
            .unwrap();
        assert_eq!(reply["result"]["isError"], true);
        let reply = server
            .handle(&json!({ "jsonrpc": "2.0", "id": 4, "method": "unknown" }))
            .unwrap();
        assert_eq!(reply["error"]["code"], -32601);
    }
}
