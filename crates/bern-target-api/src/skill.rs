//! The agent skill a build ships beside its command line: a `SKILL.md`
//! that tells an agent how to reach the running application and lists
//! every command, query, state field and UI action of this build, from
//! the same catalog the command line and the MCP server derive theirs from.
//!
//! It goes into `skills/<program>/SKILL.md`, beside `bin/`, so the folder
//! can be copied as it is into an agent's skills directory. Targets write
//! it only when the application listens on the control channel; without
//! one the programs cannot reach it.

use crate::BuildStep;
use crate::glue::program;
use bern_ir::Application;
use bern_media::media_catalog;
use bern_protocol::{CommandSpec, Retry, Type, TypeTable, session_catalog};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Folder holding the skill below the root of a build or package.
pub const DIR: &str = "skills";

/// The longest description agents read from a skill's front matter.
const DESCRIPTION_LIMIT: usize = 1024;

/// Where the skill goes below the root of a build or package:
/// `skills/<program>/SKILL.md`.
pub fn path(app: &Application) -> PathBuf {
    PathBuf::from(DIR).join(program(app)).join("SKILL.md")
}

/// How one copy of the skill finds the programs, and what reaching the
/// application takes on its target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// The command line, relative to the skill's folder.
    pub cli: String,
    /// The MCP server, relative to the skill's folder.
    pub mcp: String,
    /// Where an installed copy of the application keeps the command line,
    /// when its packages install to a known place.
    pub installed: Option<String>,
    /// What the programs need before they reach the application, besides
    /// it running, as Markdown.
    pub connect: Option<String>,
}

impl Place {
    /// The programs in `bin/` beside `skills/`, as every build lays them
    /// out; `windows` spells the paths as Windows does, with `.exe`.
    pub fn beside_bin(app: &Application, windows: bool) -> Place {
        let program = program(app);
        let (separator, suffix) = if windows { ("\\", ".exe") } else { ("/", "") };
        let path = |name: &str| ["..", "..", "bin", &format!("{name}{suffix}")].join(separator);
        Place {
            cli: path(&program),
            mcp: path(&format!("{program}-mcp")),
            installed: None,
            connect: None,
        }
    }

    pub fn installed(mut self, path: impl Into<String>) -> Place {
        self.installed = Some(path.into());
        self
    }

    pub fn connect(mut self, text: impl Into<String>) -> Place {
        self.connect = Some(text.into());
        self
    }
}

/// The steps that write the skill below `root`, or, when the target does
/// not listen (`place` is `None`), remove the one an earlier build left.
/// The folder is written afresh, so a renamed program leaves no old skill.
pub fn steps(app: &Application, root: &Path, place: Option<&Place>) -> Vec<BuildStep> {
    let mut steps = vec![BuildStep::RemoveIfExists {
        path: root.join(DIR),
    }];
    if let Some(place) = place {
        steps.push(BuildStep::Write {
            path: root.join(path(app)),
            contents: markdown(app, place).into_bytes(),
        });
    }
    steps
}

/// Whether the target's application listens on the control channel.
pub fn listens(channels: &[String]) -> bool {
    channels.iter().any(|channel| channel == "ipc")
}

/// What an Android app needs: the programs run on this computer and reach
/// the device through `adb forward`.
pub fn android_connect(app: &Application) -> String {
    let id = &app.app.bundle_id;
    let dir = format!(
        "bern-{:016x}",
        bern_protocol::fingerprint_bytes(id.as_bytes())
    );
    format!(
        "The app runs on the Android device and the programs run on this computer, which reaches the device through `adb`. `bern run --target android` forwards the control channel; when the app was started otherwise, forward it first:\n\n\
         ```bash\n\
         dir=\"${{BERN_IPC_DIR:-${{TMPDIR:-/tmp}}}}/{dir}\"\n\
         mkdir -p \"$dir\" && chmod 700 \"$dir\" && rm -f \"$dir/android.sock\"\n\
         adb forward localfilesystem:\"$dir/android.sock\" localabstract:bern.{id}\n\
         ```\n\n\
         On Windows the folder is `%TEMP%\\{dir}`."
    )
}

/// What an app in the iOS or tvOS simulator needs: it listens only when
/// started with the directory the programs use.
pub fn simulator_connect(app: &Application) -> String {
    let id = &app.app.bundle_id;
    format!(
        "The app runs in the simulator and listens only when it was started with `BERN_IPC_DIR` naming the directory the programs use. `bern run` starts it so; otherwise launch it with:\n\n\
         ```bash\n\
         SIMCTL_CHILD_BERN_IPC_DIR=\"${{BERN_IPC_DIR:-${{TMPDIR:-/tmp}}}}\" xcrun simctl launch booted {id}\n\
         ```\n\n\
         When this Mac runs the macOS build of the same app too, both answer: choose with `--engine` (the error lists their ids)."
    )
}

/// One line of text: a doc comment's lines joined with spaces.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One line of text inside a Markdown table cell.
fn cell(text: &str) -> String {
    one_line(text).replace('|', "\\|")
}

/// A string in YAML's double-quoted style, which JSON strings are.
fn yaml_string(text: &str) -> String {
    serde_json::to_string(text).expect("strings serialize")
}

/// The type of a parameter or field as the command line takes it: the
/// variants of an enum without data, else the catalog's spelling.
fn type_text(ty: &Type, types: &TypeTable) -> String {
    match ty {
        Type::Enum { name } => match types.enum_def(name) {
            Some(def) if def.is_unit_only() => def
                .variants
                .iter()
                .map(|variant| variant.name.as_str())
                .collect::<Vec<_>>()
                .join("|"),
            _ => ty.display(),
        },
        Type::Optional { inner } => type_text(inner, types),
        other => other.display(),
    }
}

/// The application's name in every language it has, its default first.
fn names(app: &Application) -> Vec<String> {
    let name = &app.app.name;
    let mut names: Vec<String> = Vec::new();
    let languages = std::iter::once(&app.languages.default).chain(&app.languages.supported);
    for text in languages
        .filter_map(|language| name.get(language))
        .chain(name.translations.values().map(String::as_str))
    {
        let text = one_line(text);
        if !text.is_empty() && !names.contains(&text) {
            names.push(text);
        }
    }
    if names.is_empty() {
        names.push(program(app));
    }
    names
}

/// The skill's front matter description: what it is for and when to use
/// it, followed by as many of the commands as fit.
fn description(app: &Application, names: &[String], cli: &str) -> String {
    let mut text = format!(
        "Operate {} ({}) while it runs, through its command line `{cli}` and its MCP server `{cli}-mcp`: every command, query, state field and UI action of this build, with their parameters. Use when asked to use, inspect, test or automate {}.",
        names.join(" / "),
        app.app.bundle_id,
        names[0],
    );
    if text.chars().count() > DESCRIPTION_LIMIT {
        text = text.chars().take(DESCRIPTION_LIMIT - 1).collect();
        text.push('…');
        return text;
    }
    // " Commands: a, b, …." within the limit.
    let room = DESCRIPTION_LIMIT - text.chars().count();
    let mut listed: Vec<String> = Vec::new();
    let mut length = " Commands: .".chars().count();
    let mut all = true;
    for spec in &app.protocol.commands {
        let command = spec.external.cli.join(" ");
        let added = command.chars().count() + if listed.is_empty() { 0 } else { 2 };
        // Leave room for ", …" after the last one that fits.
        if length + added + 3 > room {
            all = false;
            break;
        }
        length += added;
        listed.push(command);
    }
    if !listed.is_empty() {
        text.push_str(" Commands: ");
        text.push_str(&listed.join(", "));
        if !all {
            text.push_str(", …");
        }
        text.push('.');
    }
    text
}

/// Named types the listed entries use, each with the table defining it,
/// in order of first use.
struct Types<'a> {
    seen: Vec<(String, &'a TypeTable)>,
}

impl<'a> Types<'a> {
    fn add(&mut self, ty: &Type, types: &'a TypeTable) {
        match ty {
            Type::Optional { inner } | Type::List { inner } => self.add(inner, types),
            Type::Enum { name } | Type::Record { name } => {
                if self.seen.iter().any(|(seen, _)| seen == name) {
                    return;
                }
                self.seen.push((name.clone(), types));
                if let Some(def) = types.record_def(name) {
                    for field in &def.fields {
                        self.add(&field.ty, types);
                    }
                }
                if let Some(def) = types.enum_def(name) {
                    for variant in &def.variants {
                        for field in &variant.fields {
                            self.add(&field.ty, types);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn add_spec(&mut self, spec: &CommandSpec, types: &'a TypeTable) {
        for param in &spec.params {
            self.add(&param.ty, types);
        }
        self.add(&spec.result, types);
    }
}

/// Error codes every command of a group may return, said once for the
/// group rather than with each command.
fn common_errors(specs: &[CommandSpec]) -> Vec<String> {
    let Some((first, rest)) = specs.split_first() else {
        return Vec::new();
    };
    if rest.is_empty() {
        return Vec::new();
    }
    first
        .errors
        .iter()
        .filter(|code| rest.iter().all(|spec| spec.errors.contains(code)))
        .cloned()
        .collect()
}

fn codes(errors: &[String]) -> String {
    errors
        .iter()
        .map(|code| format!("`{code}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn command(
    out: &mut String,
    cli: &str,
    spec: &CommandSpec,
    types: &TypeTable,
    session: bool,
    common: &[String],
) {
    let _ = writeln!(out, "### `{cli} {}`\n", spec.external.cli.join(" "));
    let doc = spec.doc.trim();
    if !doc.is_empty() {
        let doc: Vec<&str> = doc.lines().map(str::trim).collect();
        let _ = writeln!(out, "{}\n", doc.join("\n"));
    }
    for param in &spec.params {
        let optional = matches!(param.ty, Type::Optional { .. });
        let _ = write!(
            out,
            "- `--{} <{}>`",
            param.name.replace('_', "-"),
            type_text(&param.ty, types)
        );
        if optional {
            out.push_str(" (optional; `none` for no value)");
        }
        let doc = one_line(&param.doc);
        if !doc.is_empty() {
            let _ = write!(out, ": {doc}");
        }
        out.push('\n');
    }
    if !spec.params.is_empty() {
        out.push('\n');
    }
    let mut facts = Vec::new();
    if spec.result != Type::Unit {
        facts.push(format!("Result: `{}`.", spec.result.display()));
    }
    let errors: Vec<String> = spec
        .errors
        .iter()
        .filter(|code| !common.contains(code))
        .cloned()
        .collect();
    if !errors.is_empty() {
        let also = if common.is_empty() { "" } else { "also " };
        facts.push(format!("Errors: {also}{}.", codes(&errors)));
    }
    if let Some(event) = &spec.on {
        facts.push(format!("Also runs by itself after the event `{event}`."));
    }
    if spec.policy.retry == Retry::Idempotent && !spec.is_query() {
        facts.push("Repeating it leaves the same state.".into());
    }
    let mcp = if session {
        format!(
            "MCP tool: `{}` (with an optional `session_id`).",
            spec.external.mcp
        )
    } else {
        format!("MCP tool: `{}`.", spec.external.mcp)
    };
    facts.push(mcp);
    let _ = writeln!(out, "{}\n", facts.join(" "));
}

fn type_section(out: &mut String, name: &str, types: &TypeTable) {
    if let Some(def) = types.record_def(name) {
        let _ = writeln!(
            out,
            "### `{name}`\n\nA JSON object with every one of these fields:\n\n| Field | Type | Meaning |\n| --- | --- | --- |"
        );
        for field in &def.fields {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} |",
                field.name,
                field.ty.display(),
                cell(&field.doc)
            );
        }
        out.push('\n');
    } else if let Some(def) = types.enum_def(name) {
        if def.is_unit_only() {
            let variants: Vec<String> = def
                .variants
                .iter()
                .map(|variant| format!("`\"{}\"`", variant.name))
                .collect();
            let _ = writeln!(out, "### `{name}`\n\nOne of {}.\n", variants.join(", "));
        } else {
            let _ = writeln!(
                out,
                "### `{name}`\n\nA variant name as a JSON string, or for a variant with fields an object `{{\"Variant\": {{fields}}}}`:\n"
            );
            for variant in &def.variants {
                if variant.fields.is_empty() {
                    let _ = writeln!(out, "- `\"{}\"`", variant.name);
                } else {
                    let fields: Vec<String> = variant
                        .fields
                        .iter()
                        .map(|field| format!("`{}`: `{}`", field.name, field.ty.display()))
                        .collect();
                    let _ = writeln!(out, "- `{}` with {}", variant.name, fields.join(", "));
                }
            }
            out.push('\n');
        }
    }
}

/// The skill's `SKILL.md`.
pub fn markdown(app: &Application, place: &Place) -> String {
    let cli = program(app);
    let names = names(app);
    let protocol = &app.protocol;
    let (media_types, media_specs) = media_catalog();
    let (session_types, session_specs) = session_catalog();
    let media = !app.players.is_empty();
    let commands: Vec<&CommandSpec> = protocol
        .commands
        .iter()
        .filter(|spec| !spec.is_query())
        .collect();
    let queries: Vec<&CommandSpec> = protocol
        .commands
        .iter()
        .filter(|spec| spec.is_query())
        .collect();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "---\nname: {cli}\ndescription: {}\n---\n",
        yaml_string(&description(app, &names, &cli))
    );
    let _ = writeln!(out, "# {}\n", names[0]);
    let _ = writeln!(
        out,
        "{} (`{}`, version {}) is built with Bern: everything it does is a command of one catalog, and its command line and MCP server reach the running app through the same catalog its own controls use. This file lists every command of this build and was generated with it; a new build brings a new copy.\n",
        names.join(" / "),
        app.app.bundle_id,
        one_line(&app.app.version)
    );

    out.push_str(
        "## Programs\n\n| Program | Relative to this file | What it is |\n| --- | --- | --- |\n",
    );
    let _ = writeln!(
        out,
        "| `{cli}` | `{}` | Command line: one command per run |",
        place.cli
    );
    let _ = writeln!(
        out,
        "| `{cli}-mcp` | `{}` | MCP server on standard input and output |\n",
        place.mcp
    );
    if let Some(installed) = &place.installed {
        let _ = writeln!(
            out,
            "An installed copy of the app keeps the command line at `{installed}`, the MCP server beside it.\n"
        );
    }
    let _ = writeln!(
        out,
        "Below, `{cli}` stands for the command line's path. Both programs talk to the app that is already running and never start it or a second copy: start the app first. `{cli} state` shows whether they reach it.\n"
    );
    if let Some(connect) = &place.connect {
        let _ = writeln!(out, "{}\n", connect.trim_end());
    }

    let _ = writeln!(
        out,
        "## Calling commands\n\n```\n{cli} [--engine ID] [--session ID] <command> [--parameter value]...\n```\n"
    );
    let _ = writeln!(
        out,
        "- Write parameters as `--name value` or `--name=value`. Text goes as it is; `int` and `float` as numbers; `bool` as `true` or `false`; an optional parameter takes `none` for no value; a choice takes its variant name. Lists, records and variants with fields go as JSON (see Types below).\n\
         - Each run waits for the result and prints it as JSON. Exit status: 0 done, 1 the command failed (`error[code]: message` on standard error), 2 wrong usage, 3 the app is not running or the connection was lost.\n\
         - `--engine ID` picks the app when several copies run; the error lists their ids. `--session ID` picks the window session for `ui` and `window` commands when several are open.\n\
         - `{cli} state` prints the state below, `{cli} sessions` the open UI sessions and windows, `{cli} catalog` the whole catalog as JSON, `{cli} --help` the commands.\n"
    );

    let mcp_file = place.mcp.rsplit(['/', '\\']).next().unwrap_or(&place.mcp);
    let _ = writeln!(
        out,
        "## MCP server\n\n`{cli}-mcp` speaks MCP over standard input and output. Register it with its absolute path, for example:\n\n```json\n{{ \"mcpServers\": {{ \"{cli}\": {{ \"command\": \"/absolute/path/to/{mcp_file}\" }} }} }}\n```\n"
    );
    let _ = writeln!(
        out,
        "Its tools are the commands below under their **MCP tool** names, taking the parameters as a JSON object with the names written with `_`. `bern_state` reads the state and `bern_sessions` lists the UI sessions.\n"
    );

    let mut types = Types { seen: Vec::new() };
    for field in &protocol.state {
        types.add(&field.ty, &protocol.types);
    }
    for spec in &protocol.commands {
        types.add_spec(spec, &protocol.types);
    }
    if media {
        for spec in &media_specs {
            types.add_spec(spec, &media_types);
        }
    }
    for spec in &session_specs {
        types.add_spec(spec, &session_types);
    }

    if !protocol.state.is_empty() {
        let _ = writeln!(
            out,
            "## State\n\n`{cli} state` prints `{{\"revision\": n, \"values\": {{…}}}}` with these fields:\n\n| Field | Type | Meaning |\n| --- | --- | --- |"
        );
        for field in &protocol.state {
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {} |",
                field.name,
                field.ty.display(),
                cell(&field.doc)
            );
        }
        out.push('\n');
    }

    if !commands.is_empty() {
        out.push_str(
            "## Commands\n\nThese change the app's state, with or without a window open.\n\n",
        );
        for spec in &commands {
            command(&mut out, &cli, spec, &protocol.types, false, &[]);
        }
    }
    if !queries.is_empty() {
        out.push_str("## Queries\n\nThese only read and change nothing.\n\n");
        for spec in &queries {
            command(&mut out, &cli, spec, &protocol.types, false, &[]);
        }
    }

    if media {
        out.push_str(
            "## Media\n\nThe app's players, which these commands act on by `player_id`:\n\n",
        );
        for player in &app.players {
            let doc = one_line(&player.doc);
            if doc.is_empty() {
                let _ = writeln!(out, "- `{}`", player.id);
            } else {
                let _ = writeln!(out, "- `{}`: {doc}", player.id);
            }
        }
        out.push('\n');
        let common = common_errors(&media_specs);
        if !common.is_empty() {
            let _ = writeln!(out, "Each of them may fail with {}.\n", codes(&common));
        }
        for spec in &media_specs {
            command(&mut out, &cli, spec, &media_types, false, &common);
        }
    }

    out.push_str(
        "## Driving the UI\n\nThese act on an open window through its real controls, as a person would: a hidden or disabled control refuses, and without an open window they fail with `requires_ui_session`. Start with `ui inspect`, which lists the mounted controls with the `target` id each other command takes. A control written in a component is `component.control`; one written directly in a window takes the window's id, such as `main.add`; a control repeated for each item adds the item keys, outermost first, such as `card.open[3]`.\n\n",
    );
    if !app.windows.windows.is_empty() {
        out.push_str("Window kinds, for `window open --kind`:\n\n");
        for window in &app.windows.windows {
            let title = window
                .title
                .get(&app.languages.default)
                .map(one_line)
                .unwrap_or_default();
            let primary = if window.id == app.windows.primary {
                " (the main window)"
            } else {
                ""
            };
            if title.is_empty() {
                let _ = writeln!(out, "- `{}`{primary}", window.id);
            } else {
                let _ = writeln!(out, "- `{}`{primary}: {title}", window.id);
            }
        }
        out.push('\n');
    }
    let languages: Vec<String> = app
        .languages
        .supported
        .iter()
        .map(|language| format!("`{language}`"))
        .collect();
    let _ = writeln!(
        out,
        "Interface languages, for `ui set-language`: {}.\n",
        languages.join(", ")
    );
    let common = common_errors(&session_specs);
    if !common.is_empty() {
        let _ = writeln!(out, "Each of them may fail with {}.\n", codes(&common));
    }
    for spec in &session_specs {
        command(&mut out, &cli, spec, &session_types, true, &common);
    }

    if !types.seen.is_empty() {
        out.push_str("## Types\n\nThe JSON forms of the values the commands take and give. `int` may also be written as a string of digits; `T?` is a `T` or `null`; `[T]` is a list.\n\n");
        for (name, table) in &types.seen {
            type_section(&mut out, name, table);
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bern_ir::{AppInfo, Languages, PlayerDef, Theme, ThemeMode, WindowCatalog};
    use bern_protocol::{
        CommandPolicy, EnumDef, ExternalNames, FieldDef, LocalizedText, OperationKind, ParamSpec,
        ProtocolDescriptor, RecordDef, Scope, StateField, VariantDef,
    };
    use std::collections::BTreeMap;

    fn spec(name: &str, kind: OperationKind, params: Vec<ParamSpec>, result: Type) -> CommandSpec {
        CommandSpec {
            id: format!("planner.{name}"),
            name: name.into(),
            doc: format!("Doc of {name}.\nSecond line."),
            kind,
            scope: Scope::Engine,
            params,
            result,
            errors: vec![],
            policy: CommandPolicy::default(),
            external: ExternalNames::derive("planner", name),
            reads: vec![],
            on: None,
        }
    }

    fn param(name: &str, ty: Type) -> ParamSpec {
        ParamSpec {
            name: name.into(),
            ty,
            doc: format!("The {name}"),
        }
    }

    fn app() -> Application {
        let priority = EnumDef {
            name: "Priority".into(),
            variants: ["Low", "High"]
                .iter()
                .map(|name| VariantDef {
                    name: (*name).into(),
                    fields: vec![],
                    label: None,
                })
                .collect(),
        };
        let note = RecordDef {
            name: "Note".into(),
            fields: vec![FieldDef {
                name: "title".into(),
                ty: Type::Text,
                doc: "Title | shown".into(),
                content: true,
            }],
        };
        let mut rename = spec(
            "rename",
            OperationKind::Command,
            vec![
                param("new_title", Type::Text),
                param(
                    "priority",
                    Type::Enum {
                        name: "Priority".into(),
                    },
                ),
            ],
            Type::Unit,
        );
        rename.errors = vec!["empty_title".into()];
        rename.policy.retry = Retry::Idempotent;
        let notes = spec(
            "notes",
            OperationKind::Query,
            vec![param("limit", Type::optional(Type::Int))],
            Type::list(Type::Record {
                name: "Note".into(),
            }),
        );
        Application {
            ir_version: bern_ir::IR_VERSION,
            app: AppInfo {
                id: "my_planner".into(),
                bundle_id: "dev.bern.planner".into(),
                name: LocalizedText::new([("en", "Planner \"Q\""), ("zh-Hans", "计划")]),
                version: "0.1.0".into(),
                icon: None,
                platform_icons: BTreeMap::new(),
                publisher: None,
            },
            languages: Languages {
                supported: vec!["zh-Hans".into(), "en".into()],
                default: "zh-Hans".into(),
                strict: true,
            },
            theme: Theme {
                mode: ThemeMode::System,
                brand_seed: None,
            },
            protocol: ProtocolDescriptor {
                descriptor_version: 1,
                namespace: "planner".into(),
                version: 1,
                types: TypeTable {
                    enums: vec![priority],
                    records: vec![note],
                },
                state: vec![StateField {
                    name: "title".into(),
                    ty: Type::Text,
                    doc: "Title of the plan.".into(),
                    content: true,
                }],
                commands: vec![rename, notes],
                events: vec![],
                modules: vec![],
            },
            types: TypeTable::default(),
            sessions: vec![],
            windows: WindowCatalog {
                handle: "Windows".into(),
                primary: "main".into(),
                windows: vec![],
                accent: None,
                appearance: None,
            },
            players: vec![],
            components: vec![],
            texts: vec![],
            resources: vec![],
            layouts: None,
            contracts: vec![],
            style: Default::default(),
        }
    }

    #[test]
    fn the_skill_lists_every_command_with_its_cli_and_mcp_names() {
        let app = app();
        let text = markdown(&app, &Place::beside_bin(&app, false));
        let front = text
            .strip_prefix("---\n")
            .unwrap()
            .split_once("\n---\n")
            .unwrap()
            .0;
        assert!(
            front.starts_with("name: my-planner\ndescription: \""),
            "{front}"
        );
        // The default language's name first; quotes escaped for YAML.
        assert!(
            front.contains("Operate 计划 / Planner \\\"Q\\\" (dev.bern.planner)"),
            "{front}"
        );
        assert!(
            front.contains("Commands: planner rename, planner notes."),
            "{front}"
        );

        assert!(text.contains("| `my-planner` | `../../bin/my-planner` |"));
        assert!(text.contains("| `my-planner-mcp` | `../../bin/my-planner-mcp` |"));
        assert!(text.contains("### `my-planner planner rename`\n\nDoc of rename.\nSecond line.\n"));
        assert!(text.contains("- `--new-title <text>`: The new_title\n"));
        assert!(text.contains("- `--priority <Low|High>`: The priority\n"));
        assert!(text.contains(
            "Errors: `empty_title`. Repeating it leaves the same state. MCP tool: `planner_rename`."
        ));
        // Queries apart from commands, optional parameters marked.
        let queries = text.find("## Queries").unwrap();
        assert!(text.find("planner rename`").unwrap() < queries);
        assert!(text.find("planner notes`").unwrap() > queries);
        assert!(text.contains("- `--limit <int>` (optional; `none` for no value): The limit\n"));
        assert!(text.contains("Result: `[Note]`."));
        // State, the framework's UI commands, and the types they use.
        assert!(text.contains("| `title` | `text` | Title of the plan. |"));
        assert!(text.contains("### `my-planner ui inspect`"));
        assert!(text.contains("MCP tool: `ui_activate` (with an optional `session_id`)."));
        // The errors every UI command shares are said once.
        assert!(
            text.contains("Each of them may fail with `requires_ui_session`, `target_not_found`")
        );
        assert!(
            text.contains(
                "Errors: also `invalid_url`, `unsupported_file`. MCP tool: `ui_set_file`"
            )
        );
        assert_eq!(text.matches("`requires_ui_session`").count(), 2, "{text}");
        assert!(text.contains("Interface languages, for `ui set-language`: `zh-Hans`, `en`."));
        assert!(text.contains("### `Note`\n\nA JSON object"));
        assert!(text.contains("| `title` | `text` | Title \\| shown |"));
        assert!(text.contains("### `Priority`\n\nOne of `\"Low\"`, `\"High\"`."));
        assert!(text.contains("### `ControlInfo`"));
        // Media only for an application with players.
        assert!(!text.contains("## Media"));
        assert!(text.ends_with(".\n") && !text.ends_with("\n\n"));
    }

    #[test]
    fn media_and_places_follow_the_target() {
        let mut app = app();
        app.players.push(PlayerDef {
            id: "main".into(),
            doc: "The video".into(),
        });
        let place = Place::beside_bin(&app, true)
            .installed("%LOCALAPPDATA%\\Programs\\Planner\\bin\\my-planner.exe")
            .connect(android_connect(&app));
        let text = markdown(&app, &place);
        assert!(text.contains("| `my-planner` | `..\\..\\bin\\my-planner.exe` |"));
        assert!(text.contains("\"command\": \"/absolute/path/to/my-planner-mcp.exe\""));
        assert!(text.contains("at `%LOCALAPPDATA%\\Programs\\Planner\\bin\\my-planner.exe`"));
        assert!(text.contains("localabstract:bern.dev.bern.planner"));
        assert!(text.contains("## Media\n\nThe app's players, which these commands act on by `player_id`:\n\n- `main`: The video\n"));
        assert!(text.contains("### `my-planner media play`"));
    }

    #[test]
    fn the_description_stays_within_what_agents_read() {
        let mut app = app();
        let template = app.protocol.commands[0].clone();
        for index in 0..200 {
            let mut spec = template.clone();
            spec.external = ExternalNames::derive("planner", &format!("command_number_{index}"));
            app.protocol.commands.push(spec);
        }
        let names = names(&app);
        let text = description(&app, &names, "my-planner");
        assert!(text.chars().count() <= DESCRIPTION_LIMIT, "{}", text.len());
        assert!(
            text.contains(" Commands: planner rename, planner notes, "),
            "{text}"
        );
        assert!(text.ends_with(", …."), "{text}");
    }

    #[test]
    fn steps_write_the_skill_only_where_the_app_listens() {
        let app = app();
        let root = Path::new("/out");
        let place = Place::beside_bin(&app, false);
        let steps = steps(&app, root, Some(&place));
        assert!(
            matches!(&steps[0], BuildStep::RemoveIfExists { path } if path == &root.join("skills"))
        );
        assert!(
            matches!(&steps[1], BuildStep::Write { path, .. } if path == &root.join("skills/my-planner/SKILL.md"))
        );
        let steps = super::steps(&app, root, None);
        assert_eq!(steps.len(), 1);
        assert!(listens(&["ipc".to_string()]) && !listens(&[]));
    }
}
