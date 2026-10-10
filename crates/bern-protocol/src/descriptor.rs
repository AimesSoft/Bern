use crate::{DESCRIPTOR_VERSION, SchemaError, Type, TypeTable, fingerprint_bytes};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Whether a command changes engine state or acts on one UI session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Engine,
    Session,
}

/// Commands perform actions; queries only read and never change state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    #[default]
    Command,
    Query,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concurrency {
    /// Requests run one after another in arrival order.
    Serial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Retry {
    /// Resending after an unknown outcome may repeat side effects.
    Never,
    /// Repeating the same request leaves the same final state.
    Idempotent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandPolicy {
    pub concurrency: Concurrency,
    pub retry: Retry,
    pub cancellable: bool,
}

impl Default for CommandPolicy {
    fn default() -> Self {
        Self {
            concurrency: Concurrency::Serial,
            retry: Retry::Never,
            cancellable: false,
        }
    }
}

/// Explicit, collision-checked names for entries other than the UI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalNames {
    /// Sub-command words, for example `["player", "seek"]`.
    pub cli: Vec<String>,
    /// MCP tool name, for example `player_seek`.
    pub mcp: String,
}

impl ExternalNames {
    pub fn derive(namespace: &str, name: &str) -> Self {
        Self {
            cli: vec![namespace.replace('_', "-"), name.replace('_', "-")],
            mcp: format!("{namespace}_{name}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParamSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Stable identifier, `namespace.name`.
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
    #[serde(default)]
    pub kind: OperationKind,
    pub scope: Scope,
    pub params: Vec<ParamSpec>,
    pub result: Type,
    /// Error codes the handler may return besides the generic ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub policy: CommandPolicy,
    pub external: ExternalNames,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateField {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc: String,
}

/// The complete, versioned description of one application protocol:
/// its types, read-only state snapshot and command catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDescriptor {
    pub descriptor_version: u32,
    pub namespace: String,
    /// Catalog version chosen by the application.
    pub version: u32,
    pub types: TypeTable,
    pub state: Vec<StateField>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptorError(pub String);

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DescriptorError {}

impl From<SchemaError> for DescriptorError {
    fn from(error: SchemaError) -> Self {
        DescriptorError(error.message)
    }
}

/// Namespaces used by framework-provided session, window and media commands.
pub const RESERVED_NAMESPACES: &[&str] = &["bern", "ui", "window", "session", "media"];

pub fn valid_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.ends_with('_')
        && !name.contains("__")
}

impl ProtocolDescriptor {
    pub fn command(&self, id: &str) -> Option<(usize, &CommandSpec)> {
        self.commands
            .iter()
            .enumerate()
            .find(|(_, spec)| spec.id == id)
    }

    pub fn state_index(&self, name: &str) -> Option<usize> {
        self.state.iter().position(|field| field.name == name)
    }

    /// Stable fingerprint of the full descriptor. Peers compare it during
    /// the handshake so a stale client cannot silently use another catalog.
    pub fn fingerprint(&self) -> u64 {
        fingerprint_bytes(
            serde_json::to_string(self)
                .expect("descriptor serializes")
                .as_bytes(),
        )
    }

    pub fn validate(&self) -> Result<(), DescriptorError> {
        if self.descriptor_version != DESCRIPTOR_VERSION {
            return Err(DescriptorError(format!(
                "descriptor version {} is not supported (expected {DESCRIPTOR_VERSION})",
                self.descriptor_version
            )));
        }
        if !valid_name(&self.namespace) {
            return Err(DescriptorError(format!(
                "invalid protocol namespace `{}`; use lower snake case",
                self.namespace
            )));
        }
        if RESERVED_NAMESPACES.contains(&self.namespace.as_str()) {
            return Err(DescriptorError(format!(
                "protocol namespace `{}` is reserved for framework commands",
                self.namespace
            )));
        }
        self.types.validate()?;
        let mut state_names = BTreeMap::new();
        for field in &self.state {
            if state_names.insert(&field.name, ()).is_some() {
                return Err(DescriptorError(format!(
                    "state field `{}` is declared twice",
                    field.name
                )));
            }
            self.types.check_type(&field.ty, &mut Vec::new())?;
        }
        let mut ids = BTreeMap::new();
        for command in &self.commands {
            if !valid_name(&command.name) {
                return Err(DescriptorError(format!(
                    "invalid command name `{}`; use lower snake case",
                    command.name
                )));
            }
            if command.id != format!("{}.{}", self.namespace, command.name) {
                return Err(DescriptorError(format!(
                    "command `{}` must use the id `{}.{}`",
                    command.name, self.namespace, command.name
                )));
            }
            if ids.insert(command.id.clone(), ()).is_some() {
                return Err(DescriptorError(format!(
                    "command `{}` is registered twice",
                    command.id
                )));
            }
            let mut params = BTreeMap::new();
            for param in &command.params {
                if params.insert(&param.name, ()).is_some() {
                    return Err(DescriptorError(format!(
                        "command `{}` repeats parameter `{}`",
                        command.id, param.name
                    )));
                }
                self.types.check_type(&param.ty, &mut Vec::new())?;
            }
            self.types.check_type(&command.result, &mut Vec::new())?;
        }
        check_external_names(self.commands.iter())?;
        Ok(())
    }
}

/// Reject catalogs where two commands would share a CLI path or MCP tool.
pub fn check_external_names<'a>(
    commands: impl IntoIterator<Item = &'a CommandSpec>,
) -> Result<(), DescriptorError> {
    let mut cli = BTreeMap::new();
    let mut mcp = BTreeMap::new();
    for command in commands {
        if let Some(other) = cli.insert(command.external.cli.join(" "), &command.id) {
            return Err(DescriptorError(format!(
                "commands `{other}` and `{}` map to the same CLI command `{}`",
                command.id,
                command.external.cli.join(" ")
            )));
        }
        if let Some(other) = mcp.insert(command.external.mcp.clone(), &command.id) {
            return Err(DescriptorError(format!(
                "commands `{other}` and `{}` map to the same MCP tool `{}`",
                command.id, command.external.mcp
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(namespace: &str, name: &str) -> CommandSpec {
        CommandSpec {
            id: format!("{namespace}.{name}"),
            name: name.into(),
            doc: String::new(),
            kind: OperationKind::Command,
            scope: Scope::Engine,
            params: vec![],
            result: Type::Unit,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive(namespace, name),
        }
    }

    fn descriptor(commands: Vec<CommandSpec>) -> ProtocolDescriptor {
        ProtocolDescriptor {
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "player".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![],
            commands,
        }
    }

    #[test]
    fn external_names_follow_the_documented_mapping() {
        let names = ExternalNames::derive("player", "seek_to");
        assert_eq!(names.cli, vec!["player", "seek-to"]);
        assert_eq!(names.mcp, "player_seek_to");
    }

    #[test]
    fn duplicate_commands_and_name_collisions_fail() {
        assert!(
            descriptor(vec![command("player", "seek"), command("player", "seek")])
                .validate()
                .is_err()
        );
        let mut clash = command("player", "pause");
        clash.external.mcp = "player_seek".into();
        assert!(
            descriptor(vec![command("player", "seek"), clash])
                .validate()
                .is_err()
        );
        descriptor(vec![command("player", "seek"), command("player", "pause")])
            .validate()
            .unwrap();
    }

    #[test]
    fn fingerprint_changes_with_catalog() {
        let a = descriptor(vec![command("player", "seek")]);
        let b = descriptor(vec![command("player", "pause")]);
        assert_ne!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint(), a.clone().fingerprint());
    }
}
