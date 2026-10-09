use serde::{Deserialize, Serialize};
use std::fmt;

/// Structured command failure returned identically to every entry point.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new("invalid_params", message)
    }

    pub fn unknown_command(command: &str) -> Self {
        Self::new("unknown_command", format!("unknown command `{command}`"))
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

impl From<SchemaError> for CommandError {
    fn from(error: SchemaError) -> Self {
        CommandError::invalid_params(error.message)
    }
}

/// A value or definition that does not match the declared schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    pub message: String,
}

impl SchemaError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SchemaError {}
