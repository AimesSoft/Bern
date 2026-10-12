//! Application CLI derived from the command catalog.
//!
//! Sub-commands, parameters and help come from the same descriptor the UI
//! and MCP server use. The CLI connects to an engine that is already
//! running; it never creates a second engine. Exit codes: 0 success,
//! 1 command failed, 2 usage error, 3 no connection.

use bern_adapter_json::{ClientMessage, ServerMessage, from_json};
use bern_media::{MEDIA_SOURCE, media_catalog};
use bern_protocol::{CommandSpec, ProtocolDescriptor, Type, TypeTable, session_catalog};
use bern_transport_ipc::Client;
use serde_json::{Map, Value as Json};

const USAGE_ERROR: i32 = 2;
const CONNECTION_ERROR: i32 = 3;

struct Options {
    engine: Option<String>,
    session: Option<String>,
    words: Vec<String>,
    params: Vec<(String, String)>,
    help: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        engine: None,
        session: None,
        words: Vec::new(),
        params: Vec::new(),
        help: false,
    };
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--help" || arg == "-h" {
            options.help = true;
        } else if let Some(name) = arg.strip_prefix("--") {
            let (name, value) = match name.split_once('=') {
                Some((name, value)) => (name.to_string(), value.to_string()),
                None => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| format!("`--{name}` needs a value"))?
                        .clone();
                    (name.to_string(), value)
                }
            };
            match name.as_str() {
                "engine" => options.engine = Some(value),
                "session" => options.session = Some(value),
                _ => options.params.push((name.replace('-', "_"), value)),
            }
        } else if options.params.is_empty() {
            options.words.push(arg.clone());
        } else {
            return Err(format!("unexpected argument `{arg}` after parameters"));
        }
        index += 1;
    }
    Ok(options)
}

/// Parse a command-line value according to its schema type.
fn value(text: &str, ty: &Type, types: &TypeTable) -> Result<Json, String> {
    let json = match ty {
        Type::Bool => match text {
            "true" | "yes" | "on" => Json::Bool(true),
            "false" | "no" | "off" => Json::Bool(false),
            _ => return Err(format!("expected true or false, found `{text}`")),
        },
        Type::Int => Json::String(text.to_string()),
        Type::Float => serde_json::from_str::<f64>(text)
            .map(Json::from)
            .map_err(|_| format!("expected a number, found `{text}`"))?,
        Type::Text => Json::String(text.to_string()),
        Type::Optional { inner } => {
            if text == "none" {
                Json::Null
            } else {
                value(text, inner, types)?
            }
        }
        Type::Enum { name } if types.enum_def(name).is_some_and(|def| def.is_unit_only()) => {
            Json::String(text.to_string())
        }
        // A media source may be given as its URL.
        Type::Record { name } if name == MEDIA_SOURCE && !text.trim_start().starts_with('{') => {
            serde_json::json!({ "url": text })
        }
        _ => serde_json::from_str(text).map_err(|error| format!("expected JSON: {error}"))?,
    };
    from_json(&json, ty, types).map_err(|error| error.message)?;
    Ok(json)
}

fn param_help(spec: &CommandSpec, types: &TypeTable) -> String {
    let mut lines = Vec::new();
    for param in &spec.params {
        let ty = match &param.ty {
            Type::Enum { name } => match types.enum_def(name) {
                Some(def) if def.is_unit_only() => def
                    .variants
                    .iter()
                    .map(|variant| variant.name.as_str())
                    .collect::<Vec<_>>()
                    .join("|"),
                _ => param.ty.display(),
            },
            other => other.display(),
        };
        let doc = if param.doc.is_empty() {
            String::new()
        } else {
            format!("  {}", param.doc)
        };
        lines.push(format!(
            "      --{} <{ty}>{doc}",
            param.name.replace('_', "-")
        ));
    }
    lines.join("\n")
}

fn help(name: &str, descriptor: &ProtocolDescriptor) -> String {
    let (session_types, session_specs) = session_catalog();
    let mut text = format!(
        "{name}: control the running `{}` engine\n\nUsage: {name} [--engine ID] [--session ID] <command> [--param value]...\n\nCommands:\n",
        descriptor.namespace
    );
    fn list(text: &mut String, spec: &CommandSpec, types: &TypeTable) {
        text.push_str(&format!("  {}", spec.external.cli.join(" ")));
        if !spec.doc.is_empty() {
            text.push_str(&format!("\n      {}", spec.doc.replace('\n', " ")));
        }
        let params = param_help(spec, types);
        if !params.is_empty() {
            text.push('\n');
            text.push_str(&params);
        }
        text.push('\n');
    }
    for spec in descriptor.commands.iter().filter(|spec| !spec.is_query()) {
        list(&mut text, spec, &descriptor.types);
    }
    let queries: Vec<_> = descriptor
        .commands
        .iter()
        .filter(|spec| spec.is_query())
        .collect();
    if !queries.is_empty() {
        text.push_str("\nQueries (read only; waits for the engine's answer):\n");
        for spec in queries {
            list(&mut text, spec, &descriptor.types);
        }
    }
    let (media_types, media_specs) = media_catalog();
    text.push_str("\nMedia commands (act on the engine's players):\n");
    for spec in &media_specs {
        list(&mut text, spec, &media_types);
    }
    text.push_str("\nUI session commands (need a running UI):\n");
    for spec in &session_specs {
        list(&mut text, spec, &session_types);
    }
    text.push_str(
        "\nOther:\n  state      Print the engine state\n  sessions   List live UI sessions\n  catalog    Print the command catalog as JSON\n",
    );
    text
}

fn print(json: &Json) {
    println!(
        "{}",
        serde_json::to_string_pretty(json).unwrap_or_else(|_| json.to_string())
    );
}

/// Run the CLI with the process arguments; returns the exit code.
pub fn run(descriptor: &ProtocolDescriptor, bundle_id: &str, name: &str) -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run_with(descriptor, bundle_id, name, &args)
}

pub fn run_with(
    descriptor: &ProtocolDescriptor,
    bundle_id: &str,
    name: &str,
    args: &[String],
) -> i32 {
    let options = match parse(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("error: {message}");
            return USAGE_ERROR;
        }
    };
    if options.help || options.words.is_empty() {
        print!("{}", help(name, descriptor));
        return if options.help { 0 } else { USAGE_ERROR };
    }
    let (session_types, session_specs) = session_catalog();
    let (media_types, media_specs) = media_catalog();
    let words = options.words.join(" ");
    enum Target<'a> {
        Engine(&'a CommandSpec),
        Media(&'a CommandSpec),
        Session(&'a CommandSpec),
        State,
        Sessions,
        Catalog,
    }
    let target = if let Some(spec) = descriptor
        .commands
        .iter()
        .find(|spec| spec.external.cli.join(" ") == words)
    {
        Target::Engine(spec)
    } else if let Some(spec) = media_specs
        .iter()
        .find(|spec| spec.external.cli.join(" ") == words)
    {
        Target::Media(spec)
    } else if let Some(spec) = session_specs
        .iter()
        .find(|spec| spec.external.cli.join(" ") == words)
    {
        Target::Session(spec)
    } else {
        match words.as_str() {
            "state" => Target::State,
            "sessions" => Target::Sessions,
            "catalog" => Target::Catalog,
            _ => {
                eprintln!("error: unknown command `{words}`; see `{name} --help`");
                return USAGE_ERROR;
            }
        }
    };
    let mut params = Map::new();
    if let Target::Engine(spec) | Target::Media(spec) | Target::Session(spec) = target {
        let types = match target {
            Target::Engine(_) => &descriptor.types,
            Target::Media(_) => &media_types,
            _ => &session_types,
        };
        for (param, text) in &options.params {
            let Some(param_spec) = spec
                .params
                .iter()
                .find(|candidate| candidate.name == *param)
            else {
                eprintln!(
                    "error: `{words}` has no parameter `--{}`",
                    param.replace('_', "-")
                );
                return USAGE_ERROR;
            };
            match value(text, &param_spec.ty, types) {
                Ok(json) => {
                    params.insert(param.clone(), json);
                }
                Err(message) => {
                    eprintln!("error: --{}: {message}", param.replace('_', "-"));
                    return USAGE_ERROR;
                }
            }
        }
        for param_spec in &spec.params {
            if !params.contains_key(&param_spec.name)
                && !matches!(param_spec.ty, Type::Optional { .. })
            {
                eprintln!(
                    "error: `{words}` needs `--{}`",
                    param_spec.name.replace('_', "-")
                );
                return USAGE_ERROR;
            }
        }
    } else if !options.params.is_empty() {
        eprintln!("error: `{words}` takes no parameters");
        return USAGE_ERROR;
    }

    let mut client = match Client::connect(
        bundle_id,
        options.engine.as_deref(),
        "cli",
        Some(descriptor.fingerprint()),
    ) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("error: {error}");
            return CONNECTION_ERROR;
        }
    };
    let engine_id = client.engine_id.clone();
    let session_id = options.session.clone();
    let reply = client.request(|request_id| match target {
        Target::Engine(spec) | Target::Media(spec) => ClientMessage::Command {
            request_id,
            engine_id,
            command: spec.id.clone(),
            params,
        },
        Target::Session(spec) => ClientMessage::SessionCommand {
            request_id,
            session_id,
            command: spec.id.clone(),
            params,
        },
        Target::State => ClientMessage::State { request_id },
        Target::Sessions => ClientMessage::Sessions { request_id },
        Target::Catalog => ClientMessage::Catalog { request_id },
    });
    match reply {
        Ok(ServerMessage::Result {
            ok: true, result, ..
        }) => {
            if let Some(result) = result.filter(|result| !result.is_null()) {
                print(&result);
            }
            0
        }
        Ok(ServerMessage::Result {
            error: Some(error), ..
        })
        | Ok(ServerMessage::Error { error, .. }) => {
            eprintln!("error[{}]: {}", error.code, error.message);
            1
        }
        Ok(ServerMessage::State {
            revision, values, ..
        }) => {
            print(&serde_json::json!({ "revision": revision, "values": values }));
            0
        }
        Ok(ServerMessage::Sessions { sessions, .. }) => {
            print(&serde_json::to_value(sessions).unwrap_or_default());
            0
        }
        Ok(ServerMessage::Catalog {
            protocol,
            session_commands,
            media_commands,
            ..
        }) => {
            print(&serde_json::json!({
                "protocol": protocol,
                "media_commands": media_commands,
                "session_commands": session_commands,
            }));
            0
        }
        Ok(other) => {
            eprintln!("error: unexpected reply {other:?}");
            1
        }
        Err(error) => {
            eprintln!("error: {error}");
            CONNECTION_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_split_into_words_and_parameters() {
        let args: Vec<String> = ["--engine", "e1", "player", "seek", "--position-ms", "90000"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let options = parse(&args).unwrap();
        assert_eq!(options.engine.as_deref(), Some("e1"));
        assert_eq!(options.words, vec!["player", "seek"]);
        assert_eq!(
            options.params,
            vec![("position_ms".to_string(), "90000".to_string())]
        );
        assert!(parse(&["a".into(), "--x".into()]).is_err());
    }

    #[test]
    fn help_lists_queries_apart_from_commands() {
        use bern_protocol::{
            CommandPolicy, DESCRIPTOR_VERSION, ExternalNames, OperationKind, ParamSpec, Scope,
        };
        let spec = |name: &str, kind| CommandSpec {
            id: format!("library.{name}"),
            name: name.into(),
            doc: format!("{name} doc"),
            kind,
            scope: Scope::Engine,
            params: vec![ParamSpec {
                name: "search_text".into(),
                ty: Type::Text,
                doc: String::new(),
            }],
            result: Type::Unit,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive("library", name),
            reads: vec![],
            on: None,
        };
        let descriptor = ProtocolDescriptor {
            modules: Vec::new(),
            events: Vec::new(),
            descriptor_version: DESCRIPTOR_VERSION,
            namespace: "library".into(),
            version: 1,
            types: TypeTable::default(),
            state: vec![],
            commands: vec![
                spec("add", OperationKind::Command),
                spec("search", OperationKind::Query),
            ],
        };
        let text = help("library-cli", &descriptor);
        let queries = text.find("Queries (read only").unwrap();
        assert!(text.find("library add").unwrap() < queries);
        assert!(text.find("library search").unwrap() > queries);
        assert!(text.contains("--search-text <text>"));
    }

    #[test]
    fn values_are_parsed_by_type() {
        let types = TypeTable::default();
        assert_eq!(
            value("12", &Type::Int, &types).unwrap(),
            Json::String("12".into())
        );
        assert!(value("1.5", &Type::Int, &types).is_err());
        assert_eq!(value("on", &Type::Bool, &types).unwrap(), Json::Bool(true));
        assert_eq!(
            value("none", &Type::optional(Type::Text), &types).unwrap(),
            Json::Null
        );
        assert!(value("[1,2]", &Type::list(Type::Int), &types).is_ok());
        let (media_types, _) = media_catalog();
        let source = Type::Record {
            name: MEDIA_SOURCE.into(),
        };
        assert_eq!(
            value("file:///a.mp4", &source, &media_types).unwrap(),
            serde_json::json!({ "url": "file:///a.mp4" })
        );
        assert!(value("{\"url\": 1}", &source, &media_types).is_err());
    }
}
