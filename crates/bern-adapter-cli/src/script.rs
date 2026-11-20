//! In-process CLI dispatcher for JavaScript `bern.cli(...)` calls.
//!
//! Routes CLI lines, argument vectors and structured command calls straight
//! into the live [`Host`], [`MediaHub`] and [`Sessions`] of the running
//! application without opening an IPC socket, enforcing each script context's
//! [`CliPolicy`].

use crate::{format_help_entry, parse, value};
use bern_adapter_json::{SessionInfo, params_from_json, state_json, to_json};
use bern_command::{CallContext, Entry, Host};
use bern_media::{MediaCommand, MediaHub, media_catalog};
use bern_protocol::{
    CommandSpec, Invocation, ProtocolDescriptor, SessionCommand, Type, session_catalog,
};
use bern_runtime::Sessions;
use bern_script::{
    CliCall, CliDispatcher, CliError, CliInput, CliPolicy, CliRegistration, CliTargetKind,
    split_line,
};
use serde_json::{Map, Value as Json, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Keeps the in-process script CLI host installed until dropped.
pub struct CliHostGuard {
    _registration: CliRegistration,
}

/// Install an in-process CLI host for `bern.cli(...)` backed by `host`, `media` and `sessions`.
pub fn install_host(
    host: Host,
    media: MediaHub,
    sessions: Sessions,
    program: impl Into<String>,
) -> CliHostGuard {
    let dispatcher = Arc::new(ScriptCliHost::new(host, media, sessions, program));
    CliHostGuard {
        _registration: bern_script::host::install_cli(dispatcher),
    }
}

/// In-process [`CliDispatcher`] backed by a live engine, media hub and UI sessions.
pub struct ScriptCliHost {
    host: Host,
    media: MediaHub,
    sessions: Sessions,
    program: String,
    next_request: AtomicU64,
}

impl ScriptCliHost {
    pub fn new(
        host: Host,
        media: MediaHub,
        sessions: Sessions,
        program: impl Into<String>,
    ) -> Self {
        Self {
            host,
            media,
            sessions,
            program: program.into(),
            next_request: AtomicU64::new(1),
        }
    }

    fn next_request_id(&self, policy: &CliPolicy) -> String {
        let seq = self.next_request.fetch_add(1, Ordering::Relaxed);
        match policy.label() {
            Some(label) => format!("script:{label}:{seq}"),
            None => format!("script:{seq}"),
        }
    }
}

enum ResolvedTarget<'a> {
    Engine(usize, &'a CommandSpec),
    Media(MediaCommand, CommandSpec),
    Session(SessionCommand, CommandSpec),
    State,
    Sessions,
    Catalog,
    Help,
}

impl ResolvedTarget<'_> {
    fn canonical_id(&self) -> &str {
        match self {
            ResolvedTarget::Engine(_, spec) => &spec.id,
            ResolvedTarget::Media(cmd, _) => cmd.id(),
            ResolvedTarget::Session(cmd, _) => cmd.id(),
            ResolvedTarget::State => "state",
            ResolvedTarget::Sessions => "sessions",
            ResolvedTarget::Catalog => "catalog",
            ResolvedTarget::Help => "help",
        }
    }

    fn kind(&self) -> CliTargetKind {
        match self {
            ResolvedTarget::Engine(_, spec) if spec.is_query() => CliTargetKind::EngineQuery,
            ResolvedTarget::Engine(_, _) => CliTargetKind::EngineCommand,
            ResolvedTarget::Media(_, _) => CliTargetKind::MediaCommand,
            ResolvedTarget::Session(_, _) => CliTargetKind::SessionCommand,
            ResolvedTarget::State => CliTargetKind::State,
            ResolvedTarget::Sessions => CliTargetKind::Sessions,
            ResolvedTarget::Catalog => CliTargetKind::Catalog,
            ResolvedTarget::Help => CliTargetKind::Help,
        }
    }
}

fn find_target<'a>(
    descriptor: &'a ProtocolDescriptor,
    media_specs: &[CommandSpec],
    session_specs: &[CommandSpec],
    query: &str,
    by_id: bool,
) -> Option<ResolvedTarget<'a>> {
    if let Some((index, spec)) = descriptor
        .commands
        .iter()
        .enumerate()
        .find(|(_, spec)| spec.external.cli.join(" ") == query || (by_id && spec.id == query))
    {
        return Some(ResolvedTarget::Engine(index, spec));
    }
    if let Some(spec) = media_specs
        .iter()
        .find(|spec| spec.external.cli.join(" ") == query || (by_id && spec.id == query))
        && let Some(cmd) = MediaCommand::from_id(&spec.id)
    {
        return Some(ResolvedTarget::Media(cmd, spec.clone()));
    }
    if let Some(spec) = session_specs
        .iter()
        .find(|spec| spec.external.cli.join(" ") == query || (by_id && spec.id == query))
        && let Some(cmd) = SessionCommand::from_id(&spec.id)
    {
        return Some(ResolvedTarget::Session(cmd, spec.clone()));
    }
    match query {
        "state" => Some(ResolvedTarget::State),
        "sessions" => Some(ResolvedTarget::Sessions),
        "catalog" => Some(ResolvedTarget::Catalog),
        "help" => Some(ResolvedTarget::Help),
        _ => None,
    }
}

fn help_for_policy(name: &str, descriptor: &ProtocolDescriptor, policy: &CliPolicy) -> String {
    let filter = policy.filters_catalog();
    let allow = |id: &str, kind: CliTargetKind| !filter || policy.allows_target(id, kind);

    let (session_types, session_specs) = session_catalog();
    let (media_types, media_specs) = media_catalog();
    let mut text = format!(
        "{name}: control the running `{}` engine\n\nUsage: bern.cli(\"[--session ID] <command> [--param value]...\")\n",
        descriptor.namespace
    );

    let commands: Vec<_> = descriptor
        .commands
        .iter()
        .filter(|spec| !spec.is_query() && allow(&spec.id, CliTargetKind::EngineCommand))
        .collect();
    if !commands.is_empty() {
        text.push_str("\nCommands:\n");
        for spec in commands {
            format_help_entry(&mut text, spec, &descriptor.types);
        }
    }

    let queries: Vec<_> = descriptor
        .commands
        .iter()
        .filter(|spec| spec.is_query() && allow(&spec.id, CliTargetKind::EngineQuery))
        .collect();
    if !queries.is_empty() {
        text.push_str("\nQueries (read only; waits for the engine's answer):\n");
        for spec in queries {
            format_help_entry(&mut text, spec, &descriptor.types);
        }
    }

    let media: Vec<_> = media_specs
        .iter()
        .filter(|spec| allow(&spec.id, CliTargetKind::MediaCommand))
        .collect();
    if !media.is_empty() {
        text.push_str("\nMedia commands (act on the engine's players):\n");
        for spec in media {
            format_help_entry(&mut text, spec, &media_types);
        }
    }

    let sessions: Vec<_> = session_specs
        .iter()
        .filter(|spec| allow(&spec.id, CliTargetKind::SessionCommand))
        .collect();
    if !sessions.is_empty() {
        text.push_str("\nUI session commands (need a running UI):\n");
        for spec in sessions {
            format_help_entry(&mut text, spec, &session_types);
        }
    }

    let mut other = Vec::new();
    if allow("state", CliTargetKind::State) {
        other.push("  state      Return the engine state");
    }
    if allow("sessions", CliTargetKind::Sessions) {
        other.push("  sessions   List live UI sessions");
    }
    if allow("catalog", CliTargetKind::Catalog) {
        other.push("  catalog    Return the command catalog as JSON");
    }
    if allow("help", CliTargetKind::Help) {
        other.push("  help       Return this help text");
    }
    if !other.is_empty() {
        text.push_str("\nOther:\n");
        for line in other {
            text.push_str(line);
            text.push('\n');
        }
    }
    text
}

impl CliDispatcher for ScriptCliHost {
    fn dispatch(
        &self,
        input: CliInput,
        policy: Arc<CliPolicy>,
        done: Box<dyn FnOnce(Result<Json, CliError>) + Send>,
    ) {
        let descriptor = self.host.descriptor();
        let (session_types, session_specs) = session_catalog();
        let (media_types, media_specs) = media_catalog();

        let ResolvedCall {
            target,
            params,
            session,
        } = match input {
            CliInput::Line { line } => {
                let tokens = match split_line(&line) {
                    Ok(tokens) => tokens,
                    Err(error) => {
                        done(Err(error));
                        return;
                    }
                };
                match resolve_argv(
                    descriptor,
                    &media_types,
                    &media_specs,
                    &session_types,
                    &session_specs,
                    &tokens,
                ) {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        done(Err(error));
                        return;
                    }
                }
            }
            CliInput::Args { args } => {
                match resolve_argv(
                    descriptor,
                    &media_types,
                    &media_specs,
                    &session_types,
                    &session_specs,
                    &args,
                ) {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        done(Err(error));
                        return;
                    }
                }
            }
            CliInput::Command {
                target,
                params,
                session,
            } => {
                let trimmed = target.trim();
                let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
                let Some(resolved) =
                    find_target(descriptor, &media_specs, &session_specs, &normalized, true)
                else {
                    done(Err(CliError::new(
                        "unknown_command",
                        format!("unknown command `{trimmed}`"),
                    )));
                    return;
                };
                if matches!(
                    resolved,
                    ResolvedTarget::State
                        | ResolvedTarget::Sessions
                        | ResolvedTarget::Catalog
                        | ResolvedTarget::Help
                ) && !params.is_empty()
                {
                    done(Err(CliError::invalid_params(format!(
                        "`{}` takes no parameters",
                        resolved.canonical_id()
                    ))));
                    return;
                }
                ResolvedCall {
                    target: resolved,
                    params,
                    session,
                }
            }
        };

        let call = CliCall {
            target: target.canonical_id().to_owned(),
            kind: target.kind(),
            params: params.clone(),
            session: session.clone(),
            label: policy.label().map(str::to_owned),
        };
        if let Err(error) = policy.check(&call) {
            done(Err(error));
            return;
        }

        match target {
            ResolvedTarget::Engine(index, spec) => {
                let typed = match params_from_json(spec, &params, &descriptor.types) {
                    Ok(typed) => typed,
                    Err(error) => {
                        done(Err(CliError::new(error.code, error.message)));
                        return;
                    }
                };
                let request_id = self.next_request_id(&policy);
                self.host.dispatch(
                    Invocation {
                        command: index,
                        params: typed,
                    },
                    CallContext {
                        entry: Entry::Script,
                        request_id,
                    },
                    move |outcome| {
                        let mapped = outcome
                            .result
                            .map(|v| to_json(&v, &spec.result, &descriptor.types))
                            .map_err(|error| CliError::new(error.code, error.message));
                        done(mapped);
                    },
                );
            }
            ResolvedTarget::Media(command, spec) => {
                let typed = match params_from_json(&spec, &params, &media_types) {
                    Ok(typed) => typed,
                    Err(error) => {
                        done(Err(CliError::new(error.code, error.message)));
                        return;
                    }
                };
                self.media.dispatch(command, typed, move |result| {
                    let mapped = result
                        .map(|v| to_json(&v, &spec.result, &media_types))
                        .map_err(|error| CliError::new(error.code, error.message));
                    done(mapped);
                });
            }
            ResolvedTarget::Session(command, spec) => {
                let typed = match params_from_json(&spec, &params, &session_types) {
                    Ok(typed) => typed,
                    Err(error) => {
                        done(Err(CliError::new(error.code, error.message)));
                        return;
                    }
                };
                self.sessions.dispatch(
                    session.as_deref(),
                    command,
                    typed,
                    Box::new(move |result| {
                        let mapped = result
                            .map(|v| to_json(&v, &spec.result, &session_types))
                            .map_err(|error| CliError::new(error.code, error.message));
                        done(mapped);
                    }),
                );
            }
            ResolvedTarget::State => {
                let (revision, values) = self.host.snapshot();
                let filtered: Vec<(usize, _)> = values
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| {
                        descriptor
                            .state
                            .get(*index)
                            .is_some_and(|field| policy.allows_state_field(&field.name))
                    })
                    .collect();
                done(Ok(json!({
                    "revision": revision.to_string(),
                    "values": state_json(descriptor, &filtered),
                })));
            }
            ResolvedTarget::Sessions => {
                let list: Vec<SessionInfo> = self
                    .sessions
                    .list()
                    .iter()
                    .map(|session| SessionInfo {
                        session_id: session.session_id(),
                        windows: session.windows(),
                    })
                    .collect();
                done(Ok(serde_json::to_value(list).unwrap_or_default()));
            }
            ResolvedTarget::Catalog => {
                let mut protocol = descriptor.clone();
                let mut media_commands = media_specs;
                let mut session_commands = session_specs;
                if policy.filters_catalog() {
                    protocol.commands.retain(|spec| {
                        let kind = if spec.is_query() {
                            CliTargetKind::EngineQuery
                        } else {
                            CliTargetKind::EngineCommand
                        };
                        policy.allows_target(&spec.id, kind)
                    });
                    let allow_state = policy.allows_target("state", CliTargetKind::State);
                    protocol
                        .state
                        .retain(|field| allow_state && policy.allows_state_field(&field.name));
                    media_commands
                        .retain(|spec| policy.allows_target(&spec.id, CliTargetKind::MediaCommand));
                    session_commands.retain(|spec| {
                        policy.allows_target(&spec.id, CliTargetKind::SessionCommand)
                    });
                }
                done(Ok(json!({
                    "protocol": protocol,
                    "media_commands": media_commands,
                    "session_commands": session_commands,
                })));
            }
            ResolvedTarget::Help => {
                done(Ok(Json::String(help_for_policy(
                    &self.program,
                    descriptor,
                    &policy,
                ))));
            }
        }
    }
}

struct ResolvedCall<'a> {
    target: ResolvedTarget<'a>,
    params: Map<String, Json>,
    session: Option<String>,
}

fn resolve_argv<'a>(
    descriptor: &'a ProtocolDescriptor,
    media_types: &bern_protocol::TypeTable,
    media_specs: &[CommandSpec],
    session_types: &bern_protocol::TypeTable,
    session_specs: &[CommandSpec],
    args: &[String],
) -> Result<ResolvedCall<'a>, CliError> {
    let options = parse(args).map_err(CliError::invalid_params)?;
    if options.help {
        return Ok(ResolvedCall {
            target: ResolvedTarget::Help,
            params: Map::new(),
            session: options.session,
        });
    }
    if options.words.is_empty() {
        return Err(CliError::invalid_params("CLI command must not be empty"));
    }
    let words = options.words.join(" ");
    let Some(target) = find_target(descriptor, media_specs, session_specs, &words, true) else {
        return Err(CliError::new(
            "unknown_command",
            format!("unknown command `{words}`"),
        ));
    };

    let mut params = Map::new();
    match &target {
        ResolvedTarget::Engine(_, spec) => {
            fill_cli_params(
                &words,
                spec,
                &descriptor.types,
                &options.params,
                &mut params,
            )?;
        }
        ResolvedTarget::Media(_, spec) => {
            fill_cli_params(&words, spec, media_types, &options.params, &mut params)?;
        }
        ResolvedTarget::Session(_, spec) => {
            fill_cli_params(&words, spec, session_types, &options.params, &mut params)?;
        }
        ResolvedTarget::State
        | ResolvedTarget::Sessions
        | ResolvedTarget::Catalog
        | ResolvedTarget::Help => {
            if !options.params.is_empty() {
                return Err(CliError::invalid_params(format!(
                    "`{words}` takes no parameters"
                )));
            }
        }
    }
    Ok(ResolvedCall {
        target,
        params,
        session: options.session,
    })
}

fn fill_cli_params(
    words: &str,
    spec: &CommandSpec,
    types: &bern_protocol::TypeTable,
    raw: &[(String, String)],
    params: &mut Map<String, Json>,
) -> Result<(), CliError> {
    for (param, text) in raw {
        let Some(param_spec) = spec
            .params
            .iter()
            .find(|candidate| candidate.name == *param)
        else {
            return Err(CliError::invalid_params(format!(
                "`{words}` has no parameter `--{}`",
                param.replace('_', "-")
            )));
        };
        let parsed = value(text, &param_spec.ty, types).map_err(|message| {
            CliError::invalid_params(format!("--{}: {message}", param.replace('_', "-")))
        })?;
        params.insert(param.clone(), parsed);
    }
    for param_spec in &spec.params {
        if !params.contains_key(&param_spec.name) && !matches!(param_spec.ty, Type::Optional { .. })
        {
            return Err(CliError::invalid_params(format!(
                "`{words}` needs `--{}`",
                param_spec.name.replace('_', "-")
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_command::{EngineProtocol, QueryAnswer, current_entry};
    use bern_protocol::{
        CommandError, CommandPolicy, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec,
        Scope, StateField, TypeTable, Value,
    };
    use bern_runtime::SessionHandler;
    use std::sync::OnceLock;
    use std::sync::mpsc;

    struct DemoEngine {
        count: i64,
        secret: String,
        last_entry: String,
    }

    fn descriptor() -> &'static ProtocolDescriptor {
        static DESC: OnceLock<ProtocolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "demo".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![
                StateField {
                    name: "count".into(),
                    ty: Type::Int,
                    doc: String::new(),
                    content: false,
                    stream: false,
                    keyed: None,
                },
                StateField {
                    name: "secret".into(),
                    ty: Type::Text,
                    doc: String::new(),
                    content: false,
                    stream: false,
                    keyed: None,
                },
                StateField {
                    name: "last_entry".into(),
                    ty: Type::Text,
                    doc: String::new(),
                    content: false,
                    stream: false,
                    keyed: None,
                },
            ],
            commands: vec![
                CommandSpec {
                    id: "demo.add".into(),
                    name: "add".into(),
                    doc: "Add to count".into(),
                    kind: OperationKind::Command,
                    scope: Scope::Engine,
                    params: vec![ParamSpec {
                        name: "by".into(),
                        ty: Type::Int,
                        doc: String::new(),
                    }],
                    result: Type::Int,
                    errors: vec![],
                    policy: CommandPolicy::default(),
                    external: ExternalNames::derive("demo", "add"),
                    reads: vec![],
                    on: None,
                },
                CommandSpec {
                    id: "demo.double".into(),
                    name: "double".into(),
                    doc: "Double count without mutating".into(),
                    kind: OperationKind::Query,
                    scope: Scope::Engine,
                    params: vec![],
                    result: Type::Int,
                    errors: vec![],
                    policy: CommandPolicy::default(),
                    external: ExternalNames::derive("demo", "double"),
                    reads: vec!["count".into()],
                    on: None,
                },
            ],
        })
    }

    impl EngineProtocol for DemoEngine {
        fn descriptor(&self) -> &'static ProtocolDescriptor {
            descriptor()
        }

        fn invoke(&mut self, invocation: &Invocation) -> Result<Value, CommandError> {
            self.last_entry = current_entry()
                .map(|entry| entry.name().to_owned())
                .unwrap_or_default();
            match invocation.command {
                0 => {
                    let Value::Int(by) = invocation.params[0] else {
                        return Err(CommandError::invalid_params("expected int"));
                    };
                    self.count += by;
                    Ok(Value::Int(self.count))
                }
                _ => Err(CommandError::unknown_command("unknown")),
            }
        }

        fn query(&self, invocation: &Invocation, answer: QueryAnswer) {
            match invocation.command {
                1 => answer.send(Ok(Value::Int(self.count * 2))),
                _ => answer.send(Err(CommandError::unknown_command("unknown"))),
            }
        }

        fn snapshot(&self) -> Vec<Value> {
            vec![
                Value::Int(self.count),
                Value::Text(self.secret.clone()),
                Value::Text(self.last_entry.clone()),
            ]
        }
    }

    struct StubSession;

    impl SessionHandler for StubSession {
        fn session_id(&self) -> String {
            "s1".into()
        }

        fn windows(&self) -> Vec<String> {
            vec!["main#1".into()]
        }

        fn handle(
            &self,
            command: SessionCommand,
            _params: Vec<Value>,
        ) -> Result<Value, CommandError> {
            match command {
                SessionCommand::Language => Ok(Value::Text("en".into())),
                _ => Err(CommandError::new("unsupported", "stub")),
            }
        }
    }

    fn run_cli(
        dispatcher: &ScriptCliHost,
        input: CliInput,
        policy: CliPolicy,
    ) -> Result<Json, CliError> {
        let (tx, rx) = mpsc::channel();
        dispatcher.dispatch(
            input,
            Arc::new(policy),
            Box::new(move |result| {
                let _ = tx.send(result);
            }),
        );
        rx.recv().unwrap()
    }

    #[test]
    fn script_cli_host_dispatches_commands_queries_media_sessions_and_filters() {
        let host = Host::start(DemoEngine {
            count: 5,
            secret: "top-secret".into(),
            last_entry: String::new(),
        });
        let media = MediaHub::new(&["main".into()]);
        let sessions = Sessions::new();
        sessions.register(Arc::new(StubSession));
        let cli = ScriptCliHost::new(host, media, sessions, "demo-app");

        // Line form runs engine command and sets Entry::Script.
        let added = run_cli(
            &cli,
            CliInput::Line {
                line: "demo add --by 7".into(),
            },
            CliPolicy::allow_all(),
        )
        .unwrap();
        assert_eq!(added, 12);

        // Structured command form runs query.
        let doubled = run_cli(
            &cli,
            CliInput::Command {
                target: "demo.double".into(),
                params: Map::new(),
                session: None,
            },
            CliPolicy::allow_all(),
        )
        .unwrap();
        assert_eq!(doubled, 24);

        // Media and session commands route in-process.
        let players = run_cli(
            &cli,
            CliInput::Line {
                line: "media players".into(),
            },
            CliPolicy::allow_all(),
        )
        .unwrap();
        assert_eq!(players.as_array().unwrap().len(), 1);

        let lang = run_cli(
            &cli,
            CliInput::Args {
                args: vec!["ui".into(), "language".into()],
            },
            CliPolicy::allow_all(),
        )
        .unwrap();
        assert_eq!(lang, "en");

        // State reflects Entry::Script and honors state_deny.
        let state = run_cli(
            &cli,
            CliInput::Line {
                line: "state".into(),
            },
            CliPolicy::allow_all().deny_state("secret"),
        )
        .unwrap();
        assert_eq!(state["values"]["count"], 12);
        assert_eq!(state["values"]["last_entry"], "script");
        assert!(state["values"].get("secret").is_none());

        // Allowlist filters catalog & help and blocks disallowed commands.
        let restricted = CliPolicy::allow_only(["@queries", "catalog", "help"]);
        let denied = run_cli(
            &cli,
            CliInput::Line {
                line: "demo add --by 1".into(),
            },
            restricted.clone(),
        )
        .unwrap_err();
        assert_eq!(denied.code, "permission_denied");

        let catalog = run_cli(
            &cli,
            CliInput::Line {
                line: "catalog".into(),
            },
            restricted.clone(),
        )
        .unwrap();
        let commands = catalog["protocol"]["commands"].as_array().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0]["id"], "demo.double");
        assert!(catalog["media_commands"].as_array().unwrap().is_empty());
        assert!(catalog["session_commands"].as_array().unwrap().is_empty());

        let help_text = run_cli(
            &cli,
            CliInput::Line {
                line: "help".into(),
            },
            restricted,
        )
        .unwrap();
        let help_str = help_text.as_str().unwrap();
        assert!(help_str.contains("demo double"));
        assert!(!help_str.contains("demo add"));
    }
}
