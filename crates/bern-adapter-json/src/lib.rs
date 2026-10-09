//! JSON encoding for control channels.
//!
//! JSON is an encoding, not a transport and not MCP. This crate maps
//! schema-typed values to JSON and back, defines the control messages and
//! derives JSON Schema for tool descriptions. Mappings are explicit:
//!
//! * `int` is a JSON number when |n| ≤ 2^53 − 1, otherwise a decimal string;
//!   decoders accept both, so no 64-bit value is silently rounded.
//! * enums are their declared variant name, or `{"Variant": {fields}}`.
//! * optionals are `null` or the value; records are objects.
//! * revisions and fingerprints are decimal strings.

use bern_protocol::{
    CommandError, CommandSpec, PROTOCOL_VERSION, ProtocolDescriptor, SchemaError, Type, TypeTable,
    Value,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json, json};

const SAFE_INTEGER: i64 = (1 << 53) - 1;

pub fn to_json(value: &Value, ty: &Type, types: &TypeTable) -> Json {
    match (value, ty) {
        (Value::Unit, _) => Json::Null,
        (Value::Bool(value), _) => Json::Bool(*value),
        (Value::Int(value), _) => {
            if (-SAFE_INTEGER..=SAFE_INTEGER).contains(value) {
                json!(value)
            } else {
                Json::String(value.to_string())
            }
        }
        (Value::Float(value), _) => json!(value),
        (Value::Text(value), _) => Json::String(value.clone()),
        (Value::Enum { variant, fields }, Type::Enum { name }) => {
            let Some(def) = types.enum_def(name) else {
                return Json::Null;
            };
            let variant_def = &def.variants[*variant as usize];
            if variant_def.fields.is_empty() {
                Json::String(variant_def.name.clone())
            } else {
                let mut object = Map::new();
                for (value, field) in fields.iter().zip(&variant_def.fields) {
                    object.insert(field.name.clone(), to_json(value, &field.ty, types));
                }
                let mut outer = Map::new();
                outer.insert(variant_def.name.clone(), Json::Object(object));
                Json::Object(outer)
            }
        }
        (Value::Optional(None), _) => Json::Null,
        (Value::Optional(Some(value)), Type::Optional { inner }) => to_json(value, inner, types),
        (Value::Record(values), Type::Record { name }) => {
            let Some(def) = types.record_def(name) else {
                return Json::Null;
            };
            Json::Object(
                values
                    .iter()
                    .zip(&def.fields)
                    .map(|(value, field)| (field.name.clone(), to_json(value, &field.ty, types)))
                    .collect(),
            )
        }
        (Value::List(items), Type::List { inner }) => Json::Array(
            items
                .iter()
                .map(|item| to_json(item, inner, types))
                .collect(),
        ),
        _ => Json::Null,
    }
}

fn mismatch(ty: &Type, json: &Json) -> SchemaError {
    SchemaError::new(format!("expected {}, found {json}", ty.display()))
}

pub fn from_json(json: &Json, ty: &Type, types: &TypeTable) -> Result<Value, SchemaError> {
    let value = match ty {
        Type::Unit => match json {
            Json::Null => Value::Unit,
            other => return Err(mismatch(ty, other)),
        },
        Type::Bool => Value::Bool(json.as_bool().ok_or_else(|| mismatch(ty, json))?),
        Type::Int => match json {
            Json::Number(number) => Value::Int(number.as_i64().ok_or_else(|| mismatch(ty, json))?),
            Json::String(text) => Value::Int(text.parse().map_err(|_| mismatch(ty, json))?),
            other => return Err(mismatch(ty, other)),
        },
        Type::Float => Value::Float(json.as_f64().ok_or_else(|| mismatch(ty, json))?),
        Type::Text => Value::Text(json.as_str().ok_or_else(|| mismatch(ty, json))?.to_string()),
        Type::Enum { name } => {
            let def = types
                .enum_def(name)
                .ok_or_else(|| SchemaError::new(format!("unknown enum `{name}`")))?;
            let (variant_name, payload) = match json {
                Json::String(variant) => (variant.as_str(), None),
                Json::Object(object) if object.len() == 1 => {
                    let (variant, payload) = object.iter().next().unwrap();
                    (variant.as_str(), Some(payload))
                }
                other => return Err(mismatch(ty, other)),
            };
            let index = def.variant_index(variant_name).ok_or_else(|| {
                SchemaError::new(format!(
                    "`{name}` has no variant `{variant_name}`; expected one of {}",
                    def.variants
                        .iter()
                        .map(|variant| variant.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
            let variant_def = &def.variants[index];
            let mut fields = Vec::new();
            match (payload, variant_def.fields.is_empty()) {
                (None, true) => {}
                (Some(Json::Object(object)), false) => {
                    for field in &variant_def.fields {
                        let value = object.get(&field.name).ok_or_else(|| {
                            SchemaError::new(format!("missing field `{}`", field.name))
                        })?;
                        fields.push(from_json(value, &field.ty, types)?);
                    }
                }
                _ => {
                    return Err(SchemaError::new(format!(
                        "`{name}::{variant_name}` has the wrong payload"
                    )));
                }
            }
            Value::Enum {
                variant: index as u32,
                fields,
            }
        }
        Type::Optional { inner } => match json {
            Json::Null => Value::Optional(None),
            other => Value::some(from_json(other, inner, types)?),
        },
        Type::Record { name } => {
            let def = types
                .record_def(name)
                .ok_or_else(|| SchemaError::new(format!("unknown record `{name}`")))?;
            let object = json.as_object().ok_or_else(|| mismatch(ty, json))?;
            for key in object.keys() {
                if def.field_index(key).is_none() {
                    return Err(SchemaError::new(format!("`{name}` has no field `{key}`")));
                }
            }
            Value::Record(
                def.fields
                    .iter()
                    .map(|field| {
                        let value = object.get(&field.name).ok_or_else(|| {
                            SchemaError::new(format!("missing field `{}`", field.name))
                        })?;
                        from_json(value, &field.ty, types)
                    })
                    .collect::<Result<_, _>>()?,
            )
        }
        Type::List { inner } => Value::List(
            json.as_array()
                .ok_or_else(|| mismatch(ty, json))?
                .iter()
                .map(|item| from_json(item, inner, types))
                .collect::<Result<_, _>>()?,
        ),
    };
    value.check(ty, types)?;
    Ok(value)
}

/// Decode named parameters for a command, rejecting unknown names.
pub fn params_from_json(
    spec: &CommandSpec,
    params: &Map<String, Json>,
    types: &TypeTable,
) -> Result<Vec<Value>, CommandError> {
    for key in params.keys() {
        if !spec.params.iter().any(|param| param.name == *key) {
            return Err(CommandError::invalid_params(format!(
                "`{}` has no parameter `{key}`",
                spec.id
            )));
        }
    }
    spec.params
        .iter()
        .map(|param| {
            let json = params.get(&param.name).unwrap_or(&Json::Null);
            if json.is_null() && !matches!(param.ty, Type::Optional { .. } | Type::Unit) {
                return Err(CommandError::invalid_params(format!(
                    "`{}` needs the parameter `{}`",
                    spec.id, param.name
                )));
            }
            from_json(json, &param.ty, types).map_err(|error| {
                CommandError::invalid_params(format!("`{}`: {}", param.name, error.message))
            })
        })
        .collect()
}

/// JSON Schema describing values of `ty`.
pub fn json_schema(ty: &Type, types: &TypeTable) -> Json {
    match ty {
        Type::Unit => json!({ "type": "null" }),
        Type::Bool => json!({ "type": "boolean" }),
        Type::Int => json!({
            "anyOf": [{ "type": "integer" }, { "type": "string", "pattern": "^-?[0-9]+$" }]
        }),
        Type::Float => json!({ "type": "number" }),
        Type::Text => json!({ "type": "string" }),
        Type::Optional { inner } => {
            json!({ "anyOf": [json_schema(inner, types), { "type": "null" }] })
        }
        Type::List { inner } => json!({ "type": "array", "items": json_schema(inner, types) }),
        Type::Record { name } => match types.record_def(name) {
            Some(def) => {
                let mut properties = Map::new();
                for field in &def.fields {
                    let mut schema = json_schema(&field.ty, types);
                    if !field.doc.is_empty() {
                        schema["description"] = Json::String(field.doc.clone());
                    }
                    properties.insert(field.name.clone(), schema);
                }
                let required: Vec<_> = def.fields.iter().map(|field| field.name.clone()).collect();
                json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                    "additionalProperties": false
                })
            }
            None => json!({}),
        },
        Type::Enum { name } => match types.enum_def(name) {
            Some(def) if def.is_unit_only() => json!({
                "type": "string",
                "enum": def.variants.iter().map(|variant| variant.name.clone()).collect::<Vec<_>>()
            }),
            Some(def) => json!({
                "oneOf": def.variants.iter().map(|variant| {
                    if variant.fields.is_empty() {
                        json!({ "const": variant.name })
                    } else {
                        let mut properties = Map::new();
                        for field in &variant.fields {
                            properties.insert(field.name.clone(), json_schema(&field.ty, types));
                        }
                        json!({
                            "type": "object",
                            "properties": { variant.name.clone(): {
                                "type": "object",
                                "properties": properties,
                                "required": variant.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>()
                            }},
                            "required": [variant.name],
                            "additionalProperties": false
                        })
                    }
                }).collect::<Vec<_>>()
            }),
            None => json!({}),
        },
    }
}

/// Input schema of a command, with parameter documentation.
pub fn command_input_schema(spec: &CommandSpec, types: &TypeTable) -> Json {
    let mut properties = Map::new();
    let mut required = Vec::new();
    for param in &spec.params {
        let mut schema = json_schema(&param.ty, types);
        if !param.doc.is_empty() {
            schema["description"] = Json::String(param.doc.clone());
        }
        properties.insert(param.name.clone(), schema);
        if !matches!(param.ty, Type::Optional { .. }) {
            required.push(param.name.clone());
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

/// Messages sent by entries to the engine host.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        protocol_version: u32,
        entry: String,
        #[serde(default)]
        catalog_fingerprint: Option<String>,
    },
    Command {
        request_id: String,
        engine_id: String,
        command: String,
        #[serde(default)]
        params: Map<String, Json>,
    },
    SessionCommand {
        request_id: String,
        #[serde(default)]
        session_id: Option<String>,
        command: String,
        #[serde(default)]
        params: Map<String, Json>,
    },
    State {
        request_id: String,
    },
    Catalog {
        request_id: String,
    },
    Sessions {
        request_id: String,
    },
    Subscribe {
        request_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub windows: Vec<String>,
}

/// Messages sent by the engine host to entries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        protocol_version: u32,
        engine_id: String,
        namespace: String,
        catalog_version: u32,
        catalog_fingerprint: String,
    },
    Result {
        request_id: String,
        status: String,
        revision: String,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        result: Option<Json>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<CommandError>,
    },
    State {
        request_id: String,
        revision: String,
        values: Map<String, Json>,
    },
    Catalog {
        request_id: String,
        protocol: Box<ProtocolDescriptor>,
        session_commands: Vec<CommandSpec>,
        session_types: TypeTable,
    },
    Sessions {
        request_id: String,
        sessions: Vec<SessionInfo>,
    },
    Update {
        revision: String,
        changes: Map<String, Json>,
    },
    Error {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        error: CommandError,
    },
}

/// The welcome message for a descriptor.
pub fn welcome(engine_id: &str, descriptor: &ProtocolDescriptor) -> ServerMessage {
    ServerMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        engine_id: engine_id.to_string(),
        namespace: descriptor.namespace.clone(),
        catalog_version: descriptor.version,
        catalog_fingerprint: descriptor.fingerprint().to_string(),
    }
}

/// State values keyed by field name.
pub fn state_json(descriptor: &ProtocolDescriptor, values: &[(usize, Value)]) -> Map<String, Json> {
    values
        .iter()
        .filter_map(|(index, value)| {
            let field = descriptor.state.get(*index)?;
            Some((
                field.name.clone(),
                to_json(value, &field.ty, &descriptor.types),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_protocol::{EnumDef, FieldDef, RecordDef, VariantDef};

    fn types() -> TypeTable {
        TypeTable {
            enums: vec![EnumDef {
                name: "Route".into(),
                variants: vec![
                    VariantDef {
                        name: "Home".into(),
                        fields: vec![],
                        label: None,
                    },
                    VariantDef {
                        name: "Item".into(),
                        fields: vec![FieldDef {
                            name: "id".into(),
                            ty: Type::Int,
                            doc: String::new(),
                        }],
                        label: None,
                    },
                ],
            }],
            records: vec![RecordDef {
                name: "Row".into(),
                fields: vec![FieldDef {
                    name: "route".into(),
                    ty: Type::Enum {
                        name: "Route".into(),
                    },
                    doc: String::new(),
                }],
            }],
        }
    }

    #[test]
    fn values_round_trip_through_json() {
        let types = types();
        let ty = Type::list(Type::Record { name: "Row".into() });
        let value = Value::List(vec![
            Value::Record(vec![Value::unit_variant(0)]),
            Value::Record(vec![Value::Enum {
                variant: 1,
                fields: vec![Value::Int(9_007_199_254_740_993)],
            }]),
        ]);
        let json = to_json(&value, &ty, &types);
        assert_eq!(
            json,
            json!([{ "route": "Home" }, { "route": { "Item": { "id": "9007199254740993" } } }])
        );
        assert_eq!(from_json(&json, &ty, &types).unwrap(), value);
    }

    #[test]
    fn decoding_rejects_unknown_names_and_wrong_types() {
        let types = types();
        let route = Type::Enum {
            name: "Route".into(),
        };
        assert!(from_json(&json!("Away"), &route, &types).is_err());
        assert!(
            from_json(
                &json!({ "route": "Home", "x": 1 }),
                &Type::Record { name: "Row".into() },
                &types
            )
            .is_err()
        );
        assert!(from_json(&json!(1.5), &Type::Int, &types).is_err());
        assert_eq!(
            from_json(&json!("42"), &Type::Int, &types).unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn messages_use_tagged_snake_case() {
        let message: ClientMessage = serde_json::from_value(json!({
            "type": "command",
            "request_id": "1",
            "engine_id": "e",
            "command": "player.seek",
            "params": { "position_ms": 90000 }
        }))
        .unwrap();
        assert!(matches!(message, ClientMessage::Command { .. }));
        let schema = json_schema(
            &Type::Enum {
                name: "Route".into(),
            },
            &types(),
        );
        assert!(schema["oneOf"].is_array());
    }
}
