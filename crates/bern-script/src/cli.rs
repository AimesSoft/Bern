//! In-process CLI invocation contract and allow/deny policy for JavaScript contexts.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use std::sync::Arc;

/// Default timeout for one `bern.cli(...)` invocation, in milliseconds.
pub const DEFAULT_CLI_TIMEOUT_MS: u64 = 10_000;

/// Default maximum number of concurrent `bern.cli(...)` calls per context.
pub const DEFAULT_MAX_IN_FLIGHT: usize = 32;

/// Input passed from JavaScript to `bern.cli(...)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CliInput {
    /// A single command line, split with shell-style quoting.
    Line { line: String },
    /// Pre-split CLI tokens (command words and `--param value` flags).
    Args { args: Vec<String> },
    /// Canonical command id (or CLI path) with structured JSON parameters.
    Command {
        target: String,
        #[serde(default)]
        params: Map<String, Value>,
        #[serde(default)]
        session: Option<String>,
    },
}

/// Category of a resolved CLI target, used by policy rules and interceptors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliTargetKind {
    EngineCommand,
    EngineQuery,
    MediaCommand,
    SessionCommand,
    State,
    Sessions,
    Catalog,
    Help,
}

/// Resolved CLI invocation passed to dynamic policy interceptors before execution.
#[derive(Clone, Debug)]
pub struct CliCall {
    /// Canonical target id, e.g. `media.seek`, `ui.activate`, `state`, `catalog`, `help`.
    pub target: String,
    pub kind: CliTargetKind,
    pub params: Map<String, Value>,
    pub session: Option<String>,
    pub label: Option<String>,
}

/// Structured error returned to JavaScript when `bern.cli(...)` fails.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliError {
    pub code: String,
    pub message: String,
}

impl CliError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new("permission_denied", message)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new("invalid_params", message)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CliError {}

/// Optional host-side dynamic interceptor invoked after static allow/deny rules pass.
pub type CliFilter = Arc<dyn Fn(&CliCall) -> std::result::Result<(), String> + Send + Sync>;

/// Allowlist / denylist policy controlling what a JavaScript context may call through `bern.cli`.
///
/// The default policy is [`CliPolicy::allow_all`], granting full access to the application's CLI
/// catalog. Applications that run untrusted plugins can restrict access with [`CliPolicy::deny`]
/// (denylist mode) or [`CliPolicy::allow_only`] / [`CliPolicy::deny_all`] (allowlist mode).
///
/// Deny rules always take precedence over allow rules. Patterns match canonical target ids:
/// - `"*"` matches every target;
/// - `"<namespace>.*"` matches every target in that namespace (e.g. `"media.*"`, `"ui.*"`,
///   `"window.*"`, `"trace.*"`);
/// - `"@queries"` matches all read-only engine queries;
/// - `"@engine"` matches all engine commands and queries;
/// - `"@media"` matches all media commands;
/// - `"@session"` matches all UI/window/trace session commands;
/// - `"@meta"` matches `state`, `sessions`, `catalog` and `help`;
/// - any other string matches the canonical id exactly (e.g. `"media.seek"`, `"state"`).
#[derive(Clone)]
pub struct CliPolicy {
    allow: Vec<String>,
    deny: Vec<String>,
    state_allow: Option<Vec<String>>,
    state_deny: Vec<String>,
    filter_catalog: bool,
    filter: Option<CliFilter>,
    label: Option<String>,
    max_in_flight: usize,
    timeout_ms: u64,
}

impl fmt::Debug for CliPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CliPolicy")
            .field("allow", &self.allow)
            .field("deny", &self.deny)
            .field("state_allow", &self.state_allow)
            .field("state_deny", &self.state_deny)
            .field("filter_catalog", &self.filter_catalog)
            .field("has_filter", &self.filter.is_some())
            .field("label", &self.label)
            .field("max_in_flight", &self.max_in_flight)
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

impl Default for CliPolicy {
    fn default() -> Self {
        Self::allow_all()
    }
}

impl CliPolicy {
    /// Allow every command, query, media operation, UI session command and metadata target.
    pub fn allow_all() -> Self {
        Self {
            allow: vec!["*".into()],
            deny: Vec::new(),
            state_allow: None,
            state_deny: Vec::new(),
            filter_catalog: true,
            filter: None,
            label: None,
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
            timeout_ms: DEFAULT_CLI_TIMEOUT_MS,
        }
    }

    /// Deny every target until explicit allow rules are added.
    pub fn deny_all() -> Self {
        Self {
            allow: Vec::new(),
            deny: Vec::new(),
            state_allow: None,
            state_deny: Vec::new(),
            filter_catalog: true,
            filter: None,
            label: None,
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
            timeout_ms: DEFAULT_CLI_TIMEOUT_MS,
        }
    }

    /// Allow only targets matching `patterns` (strict allowlist mode).
    pub fn allow_only<I, S>(patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::deny_all().allow_many(patterns)
    }

    /// Add one allow pattern.
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        if !pattern.is_empty() && !self.allow.iter().any(|existing| existing == &pattern) {
            self.allow.push(pattern);
        }
        self
    }

    /// Add multiple allow patterns.
    pub fn allow_many<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for pattern in patterns {
            self = self.allow(pattern);
        }
        self
    }

    /// Add one deny pattern (takes precedence over any allow pattern).
    pub fn deny(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        if !pattern.is_empty() && !self.deny.iter().any(|existing| existing == &pattern) {
            self.deny.push(pattern);
        }
        self
    }

    /// Add multiple deny patterns (each takes precedence over any allow pattern).
    pub fn deny_many<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for pattern in patterns {
            self = self.deny(pattern);
        }
        self
    }

    /// Restrict `state` output to fields matching the allowed patterns.
    pub fn allow_state(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        if !pattern.is_empty() {
            let list = self.state_allow.get_or_insert_with(Vec::new);
            if !list.iter().any(|existing| existing == &pattern) {
                list.push(pattern);
            }
        }
        self
    }

    /// Replace the `state` field allowlist with `patterns`.
    pub fn allow_state_only<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut list = Vec::new();
        for pattern in patterns {
            let pattern = pattern.into();
            if !pattern.is_empty() && !list.iter().any(|existing| existing == &pattern) {
                list.push(pattern);
            }
        }
        self.state_allow = Some(list);
        self
    }

    /// Hide matching state fields from `state` and `catalog` responses.
    pub fn deny_state(mut self, pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        if !pattern.is_empty() && !self.state_deny.iter().any(|existing| existing == &pattern) {
            self.state_deny.push(pattern);
        }
        self
    }

    /// Whether `catalog` and `help` are filtered to entries allowed by this policy (default: `true`).
    pub fn filter_catalog(mut self, enabled: bool) -> Self {
        self.filter_catalog = enabled;
        self
    }

    /// Attach a dynamic host-side interceptor checked after static allow/deny rules pass.
    pub fn with_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(&CliCall) -> std::result::Result<(), String> + Send + Sync + 'static,
    {
        self.filter = Some(Arc::new(filter));
        self
    }

    /// Attach an identifier for diagnostics and tracing.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        self.label = (!label.is_empty()).then_some(label);
        self
    }

    /// Set the per-call timeout in milliseconds (clamped to at least 1 ms).
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms.max(1);
        self
    }

    /// Set the maximum number of concurrent in-flight `bern.cli` calls (clamped to at least 1).
    pub fn with_max_in_flight(mut self, max_in_flight: usize) -> Self {
        self.max_in_flight = max_in_flight.max(1);
        self
    }

    /// Whether static allow/deny rules permit `target` of `kind`.
    pub fn allows_target(&self, target: &str, kind: CliTargetKind) -> bool {
        if self
            .deny
            .iter()
            .any(|pattern| matches_target(pattern, target, kind))
        {
            return false;
        }
        self.allow
            .iter()
            .any(|pattern| matches_target(pattern, target, kind))
    }

    /// Enforce static rules and any dynamic filter for `call`.
    pub fn check(&self, call: &CliCall) -> std::result::Result<(), CliError> {
        if !self.allows_target(&call.target, call.kind) {
            return Err(CliError::permission_denied(format!(
                "`{}` is not permitted by this script context's CLI policy",
                call.target
            )));
        }
        if let Some(filter) = &self.filter {
            filter(call).map_err(CliError::permission_denied)?;
        }
        Ok(())
    }

    /// Whether `field` may appear in `state` snapshots and filtered `catalog` responses.
    pub fn allows_state_field(&self, field: &str) -> bool {
        if self
            .state_deny
            .iter()
            .any(|pattern| matches_dotted(pattern, field))
        {
            return false;
        }
        match &self.state_allow {
            Some(allowed) => allowed.iter().any(|pattern| matches_dotted(pattern, field)),
            None => true,
        }
    }

    pub fn filters_catalog(&self) -> bool {
        self.filter_catalog
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }
}

fn matches_target(pattern: &str, target: &str, kind: CliTargetKind) -> bool {
    match pattern {
        "*" => true,
        "@queries" => kind == CliTargetKind::EngineQuery,
        "@engine" => matches!(
            kind,
            CliTargetKind::EngineCommand | CliTargetKind::EngineQuery
        ),
        "@media" => kind == CliTargetKind::MediaCommand,
        "@session" => kind == CliTargetKind::SessionCommand,
        "@meta" => matches!(
            kind,
            CliTargetKind::State
                | CliTargetKind::Sessions
                | CliTargetKind::Catalog
                | CliTargetKind::Help
        ),
        other => matches_dotted(other, target),
    }
}

fn matches_dotted(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        value == prefix
            || (value.starts_with(prefix) && value.as_bytes().get(prefix.len()) == Some(&b'.'))
    } else {
        pattern == value
    }
}

/// Split a CLI command line into tokens, supporting single quotes, double quotes
/// and backslash escapes.
pub fn split_line(line: &str) -> std::result::Result<Vec<String>, CliError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match quote {
            Some('\'') => {
                if ch == '\'' {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            Some('"') => match ch {
                '"' => quote = None,
                '\\' => match chars.next() {
                    Some('n') => current.push('\n'),
                    Some('r') => current.push('\r'),
                    Some('t') => current.push('\t'),
                    Some(escaped) => current.push(escaped),
                    None => {
                        return Err(CliError::invalid_params(
                            "unterminated escape sequence in CLI command",
                        ));
                    }
                },
                other => current.push(other),
            },
            Some(_) => unreachable!(),
            None => match ch {
                c if c.is_whitespace() => {
                    if in_token {
                        tokens.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                }
                '\'' | '"' => {
                    quote = Some(ch);
                    in_token = true;
                }
                '\\' => match chars.next() {
                    Some(escaped) => {
                        current.push(escaped);
                        in_token = true;
                    }
                    None => {
                        return Err(CliError::invalid_params(
                            "unterminated escape sequence in CLI command",
                        ));
                    }
                },
                other => {
                    current.push(other);
                    in_token = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err(CliError::invalid_params(
            "unterminated quote in CLI command",
        ));
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Host-side handler that executes `bern.cli(...)` calls in-process.
pub trait CliDispatcher: Send + Sync + 'static {
    fn dispatch(
        &self,
        input: CliInput,
        policy: Arc<CliPolicy>,
        done: Box<dyn FnOnce(std::result::Result<Value, CliError>) + Send>,
    );
}
