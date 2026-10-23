//! `bern`: the build tool.
//!
//! It creates projects, loads `Bern.toml`, compiles the declarations to
//! IR, resolves each target through the registry, lets the target check
//! and generate, runs the target's build plan and starts what it built. The
//! build tool is separate from the application CLI that `bern build`
//! generates.

mod doctor;
mod framework;
mod module;
mod new;
mod run;

use bern_compiler::{Compiled, Config};
use bern_target_api::{BuildContext, BuildStep, Diagnostic, Framework, Registry, Target};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod modules;

const USAGE: &str = "\
bern: create, check, build and run Bern applications

Usage:
  bern new <path> [--bundle-id ID] [--languages LIST] [--bern-path DIR]
  bern new --module <id> [--manifest PATH]
  bern run    [--manifest PATH] [--target NAME] [--release] [--port PORT] [--trace RULES]
  bern build  [--manifest PATH] [--target NAME] [--release]
  bern package [--manifest PATH] [--target NAME]
  bern check  [--manifest PATH] [--target NAME]
  bern doctor [--manifest PATH]
  bern ir     [--manifest PATH]

Commands:
  new          Create an application in <path>: a window with a counter; with
               --module, a module of the application in modules/<id>
  run          Build one target and start it (default: this computer's)
  build        Build targets (default: every one this computer can build)
  package      Make release packages to install, in target/bern/dist
  check        Check declarations, translations and feature coverage
  doctor       Show what this computer needs to build each target
  ir           Print the compiled IR as JSON

Options:
  --manifest   Path to Bern.toml (default: search from the current directory)
  --target     Target name from [targets.<name>]
  --exclude-module  Omit module ids from compilation (comma-separated; repeatable)
  --release    Optimized build
  --port       Local HTTP port, 1–65535, for browser app runs;
               omitted: choose a free port (a fixed port preserves browser storage)
  --trace      Start the app with performance tracing, such as engine,ui,slow:16ms
               or command:library.*; the same rules as BERN_TRACE and trace.start
  --bundle-id  Reverse-DNS bundle id (default: com.example.<name>)
  --languages  Languages, comma-separated, the default first (default: en,zh-Hans;
               the template has text in en, zh-Hans, zh-Hant and ja)
  --bern-path  Bern checkout the project builds with (default: the checkout this
               bern was built from, else the Git repository)

Signing (bern package; without it packages are not signed for distribution):
  BERN_MACOS_SIGNING_IDENTITY        Developer ID Application identity in the keychain
  BERN_MACOS_NOTARY_PROFILE          notarytool keychain profile, to notarize the .dmg
  BERN_WINDOWS_CERTIFICATE           SHA-1 thumbprint of a certificate in the user's store, or a .pfx
  BERN_WINDOWS_CERTIFICATE_PASSWORD  password of the .pfx
";

#[derive(Debug, Default)]
struct Options {
    command: String,
    /// `bern new <path>`.
    path: Option<PathBuf>,
    manifest: Option<PathBuf>,
    target: Option<String>,
    release: bool,
    port: Option<u16>,
    /// `bern run --trace`: trace rules for the started app.
    trace: Option<String>,
    bundle_id: Option<String>,
    languages: Option<Vec<String>>,
    bern_path: Option<PathBuf>,
    /// `bern new --module <id>`.
    module: Option<String>,
    exclude_modules: Vec<String>,
}

fn parse() -> Result<Options, String> {
    parse_args(std::env::args().skip(1))
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let command = args.next().ok_or_else(|| USAGE.to_string())?;
    if command == "--help" || command == "-h" || command == "help" {
        return Err(USAGE.to_string());
    }
    let allowed: &[&str] = match command.as_str() {
        "new" => &[
            "--bundle-id",
            "--languages",
            "--bern-path",
            "--module",
            "--manifest",
        ],
        "run" => &[
            "--manifest",
            "--target",
            "--release",
            "--port",
            "--trace",
            "--exclude-module",
        ],
        "build" => &["--manifest", "--target", "--release", "--exclude-module"],
        "package" => &["--manifest", "--target", "--exclude-module"],
        "check" => &["--manifest", "--target", "--exclude-module"],
        "doctor" => &["--manifest"],
        "ir" => &["--manifest", "--exclude-module"],
        other => return Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    let mut options = Options {
        command,
        ..Options::default()
    };
    while let Some(arg) = args.next() {
        if arg.starts_with("--") && !allowed.contains(&arg.as_str()) {
            return Err(format!(
                "`bern {}` does not take `{arg}`\n\n{USAGE}",
                options.command
            ));
        }
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--manifest" => options.manifest = Some(value()?.into()),
            "--target" => options.target = Some(value()?),
            "--release" => options.release = true,
            "--port" => {
                options.port = Some(
                    value()?
                        .parse::<u16>()
                        .ok()
                        .filter(|port| *port > 0)
                        .ok_or("--port must be an integer from 1 to 65535")?,
                );
            }
            "--trace" => {
                let rules = value()?;
                // Wrong rules fail before anything is built.
                bern_command::trace::Rules::parse(&rules)
                    .map_err(|error| format!("--trace: {error}"))?;
                options.trace = Some(rules);
            }
            "--bundle-id" => options.bundle_id = Some(value()?),
            "--languages" => {
                options.languages = Some(
                    value()?
                        .split(',')
                        .map(|language| language.trim().to_string())
                        .filter(|language| !language.is_empty())
                        .collect(),
                )
            }
            "--bern-path" => options.bern_path = Some(value()?.into()),
            "--module" => options.module = Some(value()?),
            "--exclude-module" => {
                for id in value()?.split(',').map(str::trim) {
                    if !bern_syntax::valid_id(id) {
                        return Err(format!(
                            "--exclude-module needs comma-separated module ids; invalid id `{id}`"
                        ));
                    }
                    if !options.exclude_modules.iter().any(|other| other == id) {
                        options.exclude_modules.push(id.to_string());
                    }
                }
            }
            _ if options.command == "new" && options.path.is_none() => {
                options.path = Some(arg.into())
            }
            other => return Err(format!("unexpected argument `{other}`\n\n{USAGE}")),
        }
    }
    if options.command == "new" && options.path.is_none() && options.module.is_none() {
        return Err(format!("`bern new` needs a path\n\n{USAGE}"));
    }
    Ok(options)
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(bern_backend_swiftui::SwiftUiMacos));
    registry.register(Box::new(bern_backend_swiftui::SwiftUiIos));
    registry.register(Box::new(bern_backend_swiftui::SwiftUiTvos));
    registry.register(Box::new(bern_backend_web::WebBrowser));
    registry.register(Box::new(bern_backend_gtk::GtkLinux));
    registry.register(Box::new(bern_backend_winui3::WinUi3Windows));
    registry.register(Box::new(bern_backend_compose::ComposeAndroid));
    registry
}

fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut directory = Some(start.to_path_buf());
    while let Some(current) = directory {
        let candidate = current.join("Bern.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        directory = current.parent().map(Path::to_path_buf);
    }
    None
}

/// The nearest enclosing Cargo workspace, as Cargo resolves it. A
/// checkout nested inside another workspace (such as a Git worktree inside
/// its main checkout) builds against its own crates and target directory.
fn find_workspace(start: &Path) -> Option<PathBuf> {
    let mut directory = Some(start.to_path_buf());
    while let Some(current) = directory {
        let manifest = current.join("Cargo.toml");
        if std::fs::read_to_string(&manifest)
            .is_ok_and(|text| text.lines().any(|line| line.trim() == "[workspace]"))
        {
            return Some(current);
        }
        directory = current.parent().map(Path::to_path_buf);
    }
    None
}

fn report(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        eprintln!("{diagnostic}\n");
    }
}

fn write_if_changed(path: &Path, contents: &[u8]) -> std::io::Result<bool> {
    if std::fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(true)
}

/// Remove everything below `dir/relative` that no wanted path needs.
fn prune(dir: &Path, relative: &Path, wanted: &BTreeSet<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir.join(relative))? {
        let entry = entry?;
        let path = relative.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if wanted.contains(&path) {
                // A wanted file replaces a directory of the same name.
                std::fs::remove_dir_all(entry.path())?;
            } else if wanted.iter().any(|wanted| wanted.starts_with(&path)) {
                prune(dir, &path, wanted)?;
            } else {
                std::fs::remove_dir_all(entry.path())?;
            }
        } else if !wanted.contains(&path) {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Copy the contents and permissions (an executable stays executable) of
/// `from` to `to` unless `to` already has the same size and modification
/// time; extended attributes are not copied.
fn copy_contents(from: &Path, to: &Path) -> std::io::Result<()> {
    let source = std::fs::metadata(from)?;
    let modified = source.modified()?;
    if let Ok(existing) = std::fs::metadata(to)
        && existing.is_file()
        && existing.len() == source.len()
        && existing.modified().is_ok_and(|time| time == modified)
    {
        return std::fs::set_permissions(to, source.permissions());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut input = std::fs::File::open(from)?;
    let mut output = std::fs::File::create(to)?;
    std::io::copy(&mut input, &mut output)?;
    output.set_permissions(source.permissions())?;
    output.set_modified(modified)
}

/// Copy `from` to a new file that then replaces `to`. Overwriting an
/// executable in place keeps the file and its cached code signature on
/// macOS, which then kills the rebuilt program at launch; a running old
/// version also keeps its own file.
fn replace_file(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut name = to.file_name().unwrap_or_default().to_os_string();
    name.push(".bern-new");
    let staged = to.with_file_name(name);
    std::fs::copy(from, &staged)?;
    std::fs::rename(&staged, to).inspect_err(|_| {
        let _ = std::fs::remove_file(&staged);
    })
}

fn mirror(dir: &Path, files: &[(PathBuf, PathBuf)], keep: &[PathBuf]) -> Result<(), String> {
    let wanted: BTreeSet<PathBuf> = files
        .iter()
        .map(|(_, to)| to.clone())
        .chain(keep.iter().cloned())
        .collect();
    std::fs::create_dir_all(dir)
        .and_then(|()| prune(dir, Path::new(""), &wanted))
        .map_err(|error| format!("cannot update {}: {error}", dir.display()))?;
    for (from, to) in files {
        copy_contents(from, &dir.join(to)).map_err(|error| {
            format!(
                "cannot copy {} to {}: {error}",
                from.display(),
                dir.join(to).display()
            )
        })?;
    }
    Ok(())
}

fn run_step(step: &BuildStep) -> Result<(), String> {
    match step {
        BuildStep::Run {
            description,
            program,
            args,
            cwd,
            env,
        } => {
            eprintln!("     \x1b[1;32mRunning\x1b[0m {description}");
            let status = Command::new(program)
                .args(args)
                .current_dir(cwd)
                .envs(env.iter().map(|(key, value)| (key, value)))
                .status()
                .map_err(|error| format!("cannot run `{program}`: {error}"))?;
            if !status.success() {
                return Err(format!(
                    "{description} failed ({program} exited with {status})"
                ));
            }
            Ok(())
        }
        BuildStep::CreateDir { path } => std::fs::create_dir_all(path)
            .map_err(|error| format!("cannot create {}: {error}", path.display())),
        BuildStep::Copy { from, to } => replace_file(from, to).map_err(|error| {
            format!(
                "cannot copy {} to {}: {error}",
                from.display(),
                to.display()
            )
        }),
        BuildStep::Mirror { dir, files, keep } => mirror(dir, files, keep),
        BuildStep::RemoveIfExists { path } => {
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|error| error.to_string())
            } else if path.exists() {
                std::fs::remove_file(path).map_err(|error| error.to_string())
            } else {
                Ok(())
            }
        }
        BuildStep::Write { path, contents } => write_if_changed(path, contents)
            .map(|_| ())
            .map_err(|error| format!("cannot write {}: {error}", path.display())),
    }
}

/// Run a plan's steps; false after reporting the first failure.
fn run_steps(steps: &[BuildStep]) -> bool {
    for step in steps {
        if let Err(message) = run_step(step) {
            eprintln!("error: {message}");
            return false;
        }
    }
    true
}

fn print_notes(notes: &[String]) {
    for note in notes {
        eprintln!("       \x1b[1;33mNote\x1b[0m {note}");
    }
}

fn context(
    config: &Config,
    compiled: &Compiled,
    workspace: &Path,
    target: &bern_target_api::TargetConfig,
    framework: &Framework,
    release: bool,
) -> BuildContext {
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: workspace.to_path_buf(),
        output_dir: {
            let base = workspace.join("target").join("bern").join(&target.name);
            let excluded = modules::excluded(config);
            if excluded.is_empty() {
                base
            } else {
                base.join(format!("without-{}", excluded.replace(',', "+")))
            }
        },
        target: target.clone(),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        modules: config
            .modules
            .iter()
            .zip(&compiled.module_protocols)
            .map(|(module, protocol_module)| bern_target_api::ModuleBuild {
                id: module.id.clone(),
                protocol_package: module.protocol_package.name.clone(),
                protocol_path: module.protocol_package.dir.clone(),
                protocol_module: protocol_module.clone(),
                engine_package: module.engine_package.name.clone(),
                engine_path: module.engine_package.dir.clone(),
                engine_type: module.engine_type.clone(),
                optional: module.optional,
                enabled: module.enabled,
                depends: module.depends.clone(),
            })
            .collect(),
        framework: framework.clone(),
        release,
    }
}

/// How an operating system is called in messages.
fn os_name(os: &str) -> &str {
    match os {
        "macos" => "macOS",
        "ios" => "iOS",
        "tvos" => "tvOS",
        "android" => "Android",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// Whether this computer can build `target`.
fn builds_here(target: &dyn Target) -> bool {
    let hosts = target.build_hosts();
    hosts.is_empty() || hosts.contains(&std::env::consts::OS)
}

/// Operating systems that can build `target`, for messages.
fn hosts_text(target: &dyn Target) -> String {
    target
        .build_hosts()
        .iter()
        .map(|os| os_name(os))
        .collect::<Vec<_>>()
        .join(" or ")
}

fn main() -> ExitCode {
    let options = match parse() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match options.command.as_str() {
        "new" => create(&options),
        "doctor" => doctor(&options),
        _ => build(&options),
    }
}

fn manifest(options: &Options) -> Option<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    options.manifest.clone().or_else(|| find_manifest(&cwd))
}

/// `bern new --module <id>`.
fn create_module(options: &Options, id: &str) -> ExitCode {
    let Some(manifest) = manifest(options) else {
        eprintln!(
            "error: no Bern.toml here or in a parent directory; run inside the application or pass --manifest"
        );
        return ExitCode::FAILURE;
    };
    let module = match module::NewModule::new(&manifest, id) {
        Ok(module) => module,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    match module.write() {
        Ok(notes) => {
            eprintln!(
                "    \x1b[1;32mCreated\x1b[0m module `{id}` in {}",
                module.app_dir.join(module.path()).display()
            );
            eprintln!(
                "             listed under [[modules]] in {}",
                manifest.display()
            );
            for note in notes {
                eprintln!("    note: {note}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `bern new`.
fn create(options: &Options) -> ExitCode {
    if let Some(id) = &options.module {
        return create_module(options, id);
    }
    let source = match &options.bern_path {
        Some(path) => match path.canonicalize() {
            Ok(path) if new::is_checkout(&path) => new::Source::Checkout(path),
            _ => {
                eprintln!("error: {} is not a Bern checkout", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => new::Source::default_for_this_build(),
    };
    let path = options.path.clone().expect("parsed");
    let project = match new::NewProject::new(
        path,
        options.bundle_id.clone(),
        options.languages.clone(),
        source,
    ) {
        Ok(project) => project,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(message) = project.write() {
        eprintln!("error: {message}");
        return ExitCode::FAILURE;
    }
    eprintln!(
        "    \x1b[1;32mCreated\x1b[0m {} ({}) in {}",
        project.name(),
        project.id,
        project.dir.display()
    );
    match &project.source {
        new::Source::Checkout(checkout) => {
            eprintln!("             with Bern from {}", checkout.display())
        }
        new::Source::Git(url) => eprintln!("             with Bern from {url}"),
    }
    eprintln!();
    eprintln!("Start it on this computer:");
    eprintln!();
    eprintln!("    cd {}", project.dir.display());
    eprintln!("    bern run");
    eprintln!();
    eprintln!("`bern doctor` shows what each target needs to build here.");
    ExitCode::SUCCESS
}

/// `bern doctor`.
fn doctor(options: &Options) -> ExitCode {
    let checks = doctor::checks();
    eprintln!(
        "bern {} on {} ({})",
        env!("CARGO_PKG_VERSION"),
        os_name(std::env::consts::OS),
        std::env::consts::ARCH
    );
    eprintln!();
    doctor::print(&checks);
    let missing = |check: &doctor::Check| matches!(check.status, doctor::Status::Missing(..));
    // Without a project only Rust is required; in one, every declared
    // target that builds here.
    let mut failed = checks
        .iter()
        .any(|check| check.platforms.is_empty() && missing(check));
    let Some(manifest) = manifest(options) else {
        return if failed {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    };
    eprintln!();
    let config = match bern_compiler::config::load(&manifest) {
        Ok(config) => config,
        Err(errors) => {
            report(&errors);
            return ExitCode::FAILURE;
        }
    };
    eprintln!("Project {} ({})", config.app_id, manifest.display());
    match find_workspace(&config.dir)
        .ok_or_else(|| format!("{} is not inside a Cargo workspace", config.dir.display()))
        .and_then(|workspace| framework::detect(&workspace, &config.engine_package.name))
    {
        Ok(framework) => eprintln!("    Bern from {}", describe(&framework)),
        Err(message) => {
            eprintln!("    \x1b[1;31merror\x1b[0m: {message}");
            failed = true;
        }
    }
    for target in &config.targets {
        let check = checks
            .iter()
            .find(|check| check.platforms.contains(&target.platform.as_str()));
        let state = match check.map(|check| &check.status) {
            Some(doctor::Status::Ready(_)) => "\x1b[32mbuilds here\x1b[0m".to_string(),
            Some(doctor::Status::Missing(..)) => {
                failed = true;
                format!(
                    "\x1b[31mneeds {}, see above\x1b[0m",
                    check.map_or("", |check| check.title)
                )
            }
            Some(doctor::Status::Elsewhere(os)) => format!("\x1b[2mbuilds on {os}\x1b[0m"),
            None => format!("unknown platform `{}`", target.platform),
        };
        eprintln!("    {:<12} {state}", target.name);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn describe(framework: &Framework) -> String {
    match framework {
        Framework::Path(crates) => crates.parent().unwrap_or(crates).display().to_string(),
        Framework::Git { url, reference } => match reference {
            Some((kind, value)) => format!("{url} ({kind} {value})"),
            None => url.clone(),
        },
        Framework::Registry(version) => format!("crates.io ({version})"),
    }
}

/// `bern check`, `bern build`, `bern package`, `bern run` and `bern ir`.
fn build(options: &Options) -> ExitCode {
    let Some(manifest) = manifest(options) else {
        eprintln!(
            "error: no Bern.toml found; pass --manifest, or create a project with `bern new`"
        );
        return ExitCode::from(2);
    };
    let config = match bern_compiler::config::load_excluding(&manifest, &options.exclude_modules) {
        Ok(config) => config,
        Err(errors) => {
            report(&errors);
            return ExitCode::FAILURE;
        }
    };
    let compiled = match bern_compiler::compile(&config) {
        Ok(compiled) => compiled,
        Err(errors) => {
            report(&errors);
            eprintln!("error: could not compile `{}`", config.app_id);
            return ExitCode::FAILURE;
        }
    };
    report(&compiled.warnings);
    if options.command == "ir" {
        println!(
            "{}",
            serde_json::to_string_pretty(&compiled.app).expect("IR serializes")
        );
        return ExitCode::SUCCESS;
    }
    let Some(workspace) = find_workspace(&config.dir) else {
        eprintln!(
            "error: {} is not inside a Cargo workspace",
            config.dir.display()
        );
        return ExitCode::FAILURE;
    };
    for library in &compiled.libraries {
        let path = workspace
            .join("target/bern/libraries")
            .join(format!("{}.json", library.package));
        let json = serde_json::to_vec_pretty(library).expect("descriptor serializes");
        if let Err(error) = write_if_changed(&path, &json) {
            eprintln!("error: cannot write {}: {error}", path.display());
            return ExitCode::FAILURE;
        }
        eprintln!(
            "  \x1b[1;32mDescribed\x1b[0m {} {} ({} components, sources {})",
            library.package,
            library.version,
            library.components.len(),
            library.source_fingerprint
        );
    }
    let registry = registry();
    let mut targets = Vec::new();
    for target_config in &config.targets {
        if options
            .target
            .as_ref()
            .is_some_and(|name| *name != target_config.name)
        {
            continue;
        }
        match registry.resolve(target_config) {
            Ok(target) => targets.push((target_config, target)),
            Err(error) => {
                report(&[error]);
                return ExitCode::FAILURE;
            }
        }
    }
    if targets.is_empty() {
        eprintln!(
            "error: no matching [targets.*] entry in {}",
            manifest.display()
        );
        return ExitCode::FAILURE;
    }
    // Without --target, `build` builds what this computer can, and `run`
    // starts this computer's own target, else one any computer builds.
    // Targets whose tools are missing here are skipped with what to install.
    if options.target.is_none() && options.command != "check" {
        let mut here = Vec::new();
        for (target_config, target) in targets {
            if !builds_here(target) {
                if options.command != "run" {
                    eprintln!(
                        "   \x1b[1;33mSkipping\x1b[0m {}: builds on {}",
                        target_config.name,
                        hosts_text(target)
                    );
                }
                continue;
            }
            if let Some(doctor::Check {
                title,
                status: doctor::Status::Missing(problem, fix),
                ..
            }) = doctor::check(&target_config.platform)
            {
                eprintln!(
                    "   \x1b[1;33mSkipping\x1b[0m {}: needs {title}: {problem}",
                    target_config.name
                );
                for line in fix.lines() {
                    eprintln!("            {line}");
                }
                continue;
            }
            here.push((target_config, target));
        }
        targets = here;
        if options.command == "run" {
            // This computer's own platform first, such as macOS rather than
            // the iOS simulator on a Mac, then another native target.
            let own = targets
                .iter()
                .position(|(config, _)| config.platform == std::env::consts::OS);
            let native = targets
                .iter()
                .position(|(_, target)| !target.build_hosts().is_empty());
            targets = own
                .or(native)
                .or((!targets.is_empty()).then_some(0))
                .map(|index| vec![targets.swap_remove(index)])
                .unwrap_or_default();
        }
        if targets.is_empty() {
            eprintln!(
                "error: no target in {} builds on {}; pass --target to generate one anyway",
                manifest.display(),
                os_name(std::env::consts::OS)
            );
            return ExitCode::FAILURE;
        }
    }
    if options.command == "run" && targets.len() != 1 {
        eprintln!("error: `bern run` starts one target");
        return ExitCode::FAILURE;
    }
    let signing = bern_target_api::Signing::from_env();
    let framework = if options.command == "check" {
        None
    } else {
        match framework::detect(&workspace, &config.engine_package.name) {
            Ok(framework) => Some(framework),
            Err(message) => {
                eprintln!("error: cannot tell where Bern comes from: {message}");
                return ExitCode::FAILURE;
            }
        }
    };
    for (target_config, target) in targets {
        let descriptor = target.descriptor();
        // The application as this platform shows it: named corners at
        // their radii here.
        let app = compiled.app.on_platform(&target_config.platform);
        let mut problems = target.check(&app, target_config);
        let features = bern_target_api::feature_report(&app, &target_config.platform);
        problems.extend(features.diagnostics.iter().cloned());
        report(&problems);
        if !features.rows.is_empty() {
            eprintln!(
                "   \x1b[1;32mFeatures\x1b[0m for {} ({})",
                descriptor.id, target_config.platform
            );
            for line in features.lines() {
                eprintln!("            {line}");
            }
        }
        if problems.iter().any(Diagnostic::is_error) {
            eprintln!(
                "error: `{}` cannot be built for `{}`",
                config.app_id, descriptor.id
            );
            return ExitCode::FAILURE;
        }
        eprintln!(
            "    \x1b[1;32mChecked\x1b[0m {} for {} ({} components, {} windows)",
            config.app_id,
            descriptor.id,
            compiled.app.components.len(),
            compiled.app.windows.windows.len()
        );
        let Some(framework) = &framework else {
            continue;
        };
        // What a named target needs on this computer, before any tool
        // fails with a message of its own.
        let needs = options
            .target
            .as_ref()
            .and_then(|_| doctor::check(&target_config.platform));
        match needs.map(|check| (check.title, check.status)) {
            Some((title, doctor::Status::Missing(problem, fix))) => {
                eprintln!(
                    "error: building `{}` needs {title}: {problem}",
                    target_config.name
                );
                for line in fix.lines() {
                    eprintln!("       {line}");
                }
                return ExitCode::FAILURE;
            }
            Some((_, doctor::Status::Elsewhere(os))) => {
                eprintln!("error: `{}` builds on {os}", target_config.name);
                return ExitCode::FAILURE;
            }
            _ => {}
        }
        let context = context(
            &config,
            &compiled,
            &workspace,
            target_config,
            framework,
            options.release || options.command == "package",
        );
        // The lengths the styles set for this target's platforms, on the
        // application with its named corners already at their radii.
        let app = bern_target_api::style::for_platforms(&app, target.descriptor().platforms);
        let files = match target.generate(&app, &context) {
            Ok(files) => files,
            Err(errors) => {
                report(&errors);
                return ExitCode::FAILURE;
            }
        };
        let features_json = serde_json::json!({
            "platform": target_config.platform,
            "features": features
                .rows
                .iter()
                .map(|(contract, feature, size, result)| serde_json::json!({
                    "contract": contract,
                    "feature": feature,
                    "size": size,
                    "result": result,
                }))
                .collect::<Vec<_>>(),
        });
        let mut files = files;
        if let Err(error) = modules::configure(&mut files, &config) {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
        files.push(bern_target_api::GeneratedFile::text(
            "features.json",
            serde_json::to_string_pretty(&features_json).expect("report serializes"),
        ));
        let mut changed = 0;
        for file in &files {
            match write_if_changed(&context.output_dir.join(&file.path), &file.contents) {
                Ok(true) => changed += 1,
                Ok(false) => {}
                Err(error) => {
                    eprintln!("error: cannot write {}: {error}", file.path.display());
                    return ExitCode::FAILURE;
                }
            }
        }
        eprintln!(
            "  \x1b[1;32mGenerated\x1b[0m {} files ({changed} changed) in {}",
            files.len(),
            context.output_dir.display()
        );
        let mut plan = match target.plan(&app, &context) {
            Ok(plan) => plan,
            Err(errors) => {
                report(&errors);
                return ExitCode::FAILURE;
            }
        };
        if options.command == "run"
            && let Some(run) = &plan.run
            && let Err(message) = run::validate_port(run, options.port)
        {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
        modules::environment(&mut plan.steps, &config);
        if !run_steps(&plan.steps) {
            return ExitCode::FAILURE;
        }
        for product in &plan.products {
            eprintln!("   \x1b[1;32mFinished\x1b[0m {}", product.display());
        }
        print_notes(&plan.notes);
        if options.command == "package" {
            let mut packages = match target.package(&app, &context, &signing) {
                Ok(packages) => packages,
                Err(errors) => {
                    report(&errors);
                    return ExitCode::FAILURE;
                }
            };
            modules::environment(&mut packages.steps, &config);
            if !run_steps(&packages.steps) {
                return ExitCode::FAILURE;
            }
            for product in &packages.products {
                eprintln!("   \x1b[1;32mPackaged\x1b[0m {}", product.display());
            }
            print_notes(&packages.notes);
        }
        if options.command == "run" {
            let Some(run) = &plan.run else {
                eprintln!("error: `{}` builds nothing to start", descriptor.id);
                return ExitCode::FAILURE;
            };
            return match run::start(run, options.port, options.trace.as_deref()) {
                Ok(code) => code,
                Err(message) => {
                    eprintln!("error: {message}");
                    ExitCode::FAILURE
                }
            };
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files_below(dir: &Path) -> Vec<String> {
        bern_target_api::files_below(dir)
            .unwrap()
            .into_iter()
            .map(|(_, relative)| {
                // `/`-separated on every platform, as resource names are.
                let parts: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                parts.join("/")
            })
            .collect()
    }

    #[test]
    fn mirror_ships_exactly_the_declared_files() {
        let root = std::env::temp_dir().join(format!("bern-mirror-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (project, bundle) = (root.join("project"), root.join("Resources"));
        std::fs::create_dir_all(project.join("media")).unwrap();
        std::fs::create_dir_all(bundle.join("en.lproj")).unwrap();
        std::fs::create_dir_all(bundle.join("old")).unwrap();
        std::fs::write(project.join("media/a.mp4"), b"first").unwrap();
        std::fs::write(project.join("media/b.mp4"), b"bee").unwrap();
        std::fs::write(bundle.join("en.lproj/Localizable.strings"), b"strings").unwrap();
        std::fs::write(bundle.join("old/stale.mp4"), b"stale").unwrap();
        std::fs::write(bundle.join("media"), b"was a file").unwrap();
        let files = |names: &[&str]| -> Vec<(PathBuf, PathBuf)> {
            names
                .iter()
                .map(|name| (project.join(name), PathBuf::from(name)))
                .collect()
        };
        let keep = vec![PathBuf::from("en.lproj/Localizable.strings")];

        mirror(&bundle, &files(&["media/a.mp4", "media/b.mp4"]), &keep).unwrap();
        assert_eq!(
            files_below(&bundle),
            ["en.lproj/Localizable.strings", "media/a.mp4", "media/b.mp4"]
        );
        assert_eq!(std::fs::read(bundle.join("media/a.mp4")).unwrap(), b"first");

        // A changed source is copied again; a dropped one disappears.
        std::fs::write(project.join("media/a.mp4"), b"second!").unwrap();
        mirror(&bundle, &files(&["media/a.mp4"]), &keep).unwrap();
        assert_eq!(
            files_below(&bundle),
            ["en.lproj/Localizable.strings", "media/a.mp4"]
        );
        assert_eq!(
            std::fs::read(bundle.join("media/a.mp4")).unwrap(),
            b"second!"
        );

        // A program stays executable.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(project.join("tool"), b"#!/bin/sh").unwrap();
            std::fs::set_permissions(project.join("tool"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
            mirror(&bundle, &files(&["tool"]), &keep).unwrap();
            let mode = std::fs::metadata(bundle.join("tool"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o755);
        }

        // Nothing declared: only the kept files remain.
        mirror(&bundle, &[], &keep).unwrap();
        assert_eq!(files_below(&bundle), ["en.lproj/Localizable.strings"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Run the steps of a plan that place files. What its `cargo` runs and
    /// archive would make is faked: a source a copy lacks becomes a file.
    fn place_files(steps: &[BuildStep]) {
        for step in steps {
            match step {
                BuildStep::Run { .. } => continue,
                BuildStep::Copy { from, .. } if !from.exists() => {
                    std::fs::create_dir_all(from.parent().unwrap()).unwrap();
                    std::fs::write(from, b"built").unwrap();
                }
                _ => {}
            }
            run_step(step).unwrap();
        }
    }

    #[test]
    fn a_web_ui_package_carries_only_its_own_project() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let workspace = std::env::temp_dir().join(format!("bern-web-ui-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&workspace);
        let framework = Framework::Path(root.join("crates"));
        let web = bern_backend_web::WebBrowser;
        // Both projects build into the same output directory, the gallery
        // first, with a folder of page icons left by yet another one.
        let mut built = None;
        for example in ["gallery", "planner"] {
            let config =
                bern_compiler::config::load(&root.join("examples").join(example).join("Bern.toml"))
                    .unwrap();
            let compiled = bern_compiler::compile(&config).unwrap();
            let target = config
                .targets
                .iter()
                .find(|target| target.name == "web_ui")
                .unwrap();
            let context = context(&config, &compiled, &workspace, target, &framework, false);
            for file in web.generate(&compiled.app, &context).unwrap() {
                write_if_changed(&context.output_dir.join(&file.path), &file.contents).unwrap();
            }
            write_if_changed(&context.output_dir.join("icons/site.webmanifest"), b"{}").unwrap();
            place_files(&web.plan(&compiled.app, &context).unwrap().steps);
            built = Some((compiled, context));
        }
        let (compiled, context) = built.unwrap();
        let programs: Vec<String> = ["planner", "planner-host", "planner-mcp"]
            .iter()
            .map(|name| format!("{name}{}", std::env::consts::EXE_SUFFIX))
            .collect();
        assert_eq!(files_below(&context.output_dir.join("bin")), programs);

        let package = web
            .package(
                &compiled.app,
                &context,
                &bern_target_api::Signing::default(),
            )
            .unwrap();
        place_files(&package.steps);
        let staged = files_below(&context.output_dir.join("package/planner"));
        for expected in [
            "index.html",
            "app.js",
            "app.wasm",
            "bern.css",
            "favicon.svg",
        ] {
            assert!(staged.iter().any(|path| path == expected), "{staged:?}");
        }
        let shipped: Vec<&str> = staged
            .iter()
            .filter_map(|path| path.strip_prefix("bin/"))
            .collect();
        assert_eq!(shipped, programs);
        assert!(
            staged.iter().all(|path| !path.contains("gallery")
                && !path.starts_with("resources/")
                && !path.starts_with("icons/")
                && !path.starts_with("glue/")
                && path != "build.json"
                && path != "features.json"),
            "{staged:?}"
        );
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn web_app_control_is_opt_in_and_the_package_stays_static() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let workspace =
            std::env::temp_dir().join(format!("bern-web-control-{}", std::process::id()));
        let config = bern_compiler::config::load(&root.join("examples/planner/Bern.toml")).unwrap();
        let compiled = bern_compiler::compile(&config).unwrap();
        let framework = Framework::Path(root.join("crates"));
        let web = bern_backend_web::WebBrowser;
        let mut target = config
            .targets
            .iter()
            .find(|t| t.name == "web_ui")
            .unwrap()
            .clone();
        target.name = "web_app".into();
        target.web_mode = Some("app".into());
        target.channels = vec!["ipc".into()];
        let context = context(&config, &compiled, &workspace, &target, &framework, false);
        let files = web.generate(&compiled.app, &context).unwrap();
        let source = |name: &str| {
            String::from_utf8(
                files
                    .iter()
                    .find(|f| f.path == Path::new(name))
                    .unwrap()
                    .contents
                    .clone(),
            )
            .unwrap()
        };
        assert!(source("glue/page/Cargo.toml").contains("features = [\"control\"]"));
        assert!(
            source("glue/control/src/cli.rs")
                .contains(&format!("{}.web", compiled.app.app.bundle_id))
        );
        assert!(!files.iter().any(|f| f.path.starts_with("glue/host")));
        for file in files {
            write_if_changed(&context.output_dir.join(file.path), &file.contents).unwrap();
        }
        place_files(&web.plan(&compiled.app, &context).unwrap().steps);
        assert!(
            context
                .output_dir
                .join(format!("bin/planner-mcp{}", std::env::consts::EXE_SUFFIX))
                .is_file()
        );
        place_files(
            &web.package(
                &compiled.app,
                &context,
                &bern_target_api::Signing::default(),
            )
            .unwrap()
            .steps,
        );
        let packaged = files_below(&context.output_dir.join("package/planner"));
        assert!(packaged.iter().any(|p| p == "control.js"));
        assert!(!packaged.iter().any(|p| p.starts_with("bin/")
            || p.starts_with("skills/")
            || p.starts_with("glue/")
            || p == "serve-bridge.py"
            || p == "control.json"));
        // Removing the channel also removes the local relay and native tools.
        let mut disabled = context.clone();
        disabled.target.channels.clear();
        let generated = web.generate(&compiled.app, &disabled).unwrap();
        assert!(!generated.iter().any(|f| f.path == Path::new("control.js")));
        place_files(&web.plan(&compiled.app, &disabled).unwrap().steps);
        assert!(!disabled.output_dir.join("bin").exists());
        assert!(!disabled.output_dir.join("serve-bridge.py").exists());
        let _ = std::fs::remove_dir_all(workspace);
    }

    fn parsed(args: &[&str]) -> Result<Options, String> {
        parse_args(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn each_command_takes_its_own_options() {
        let new = parsed(&[
            "new",
            "work/my_app",
            "--languages",
            "ja, en",
            "--bundle-id",
            "dev.me.app",
        ])
        .unwrap();
        assert_eq!(new.path, Some(PathBuf::from("work/my_app")));
        assert_eq!(
            new.languages,
            Some(vec!["ja".to_string(), "en".to_string()])
        );
        assert_eq!(new.bundle_id.as_deref(), Some("dev.me.app"));
        assert!(parsed(&["new"]).unwrap_err().contains("needs a path"));
        assert!(
            parsed(&["new", "a", "b"])
                .unwrap_err()
                .contains("unexpected argument `b`")
        );
        assert!(
            parsed(&["new", "a", "--release"])
                .unwrap_err()
                .contains("does not take `--release`")
        );

        let run = parsed(&["run", "--target", "web_app", "--release"]).unwrap();
        assert_eq!(run.target.as_deref(), Some("web_app"));
        assert!(run.release);
        assert!(parsed(&["check", "--release"]).is_err());
        assert!(parsed(&["doctor", "--target", "macos"]).is_err());
        assert!(
            parsed(&["build", "--target"])
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(parsed(&["deploy"]).unwrap_err().contains("unknown command"));
    }

    #[test]
    fn web_run_port_is_optional_nonzero_and_run_only() {
        assert_eq!(parsed(&["run"]).unwrap().port, None);
        for (value, port) in [("1", 1), ("4173", 4173), ("65535", 65535)] {
            assert_eq!(parsed(&["run", "--port", value]).unwrap().port, Some(port));
        }
        for value in ["0", "-1", "65536", "abc", "4.2"] {
            assert!(
                parsed(&["run", "--port", value])
                    .unwrap_err()
                    .contains("1 to 65535")
            );
        }
        assert!(
            parsed(&["run", "--port"])
                .unwrap_err()
                .contains("needs a value")
        );
        for command in ["build", "check", "package", "doctor"] {
            assert!(
                parsed(&[command, "--port", "4173"])
                    .unwrap_err()
                    .contains("does not take `--port`")
            );
        }
    }

    #[test]
    fn run_trace_rules_are_checked_before_building() {
        assert_eq!(
            parsed(&["run", "--trace", "engine,ui,slow:16ms"])
                .unwrap()
                .trace
                .as_deref(),
            Some("engine,ui,slow:16ms")
        );
        assert!(
            parsed(&["run", "--trace", "engin"])
                .unwrap_err()
                .contains("--trace: unknown trace rule `engin`")
        );
        assert!(
            parsed(&["build", "--trace", "engine"])
                .unwrap_err()
                .contains("does not take `--trace`")
        );
    }
}

#[cfg(test)]
mod module_option_tests {
    use super::*;
    #[test]
    fn every_compilation_command_accepts_repeated_and_comma_separated_exclusions() {
        for command in ["run", "build", "package", "check", "ir"] {
            let options = parse_args(
                [
                    command,
                    "--exclude-module",
                    "notes,tags",
                    "--exclude-module",
                    "tags,sync",
                ]
                .into_iter()
                .map(str::to_string),
            )
            .unwrap();
            assert_eq!(options.exclude_modules, ["notes", "tags", "sync"]);
        }
        for value in [
            "",
            "notes,",
            ",notes",
            "notes,,tags",
            "--target",
            "notes/../tags",
        ] {
            assert!(
                parse_args(
                    ["build", "--exclude-module", value]
                        .into_iter()
                        .map(str::to_string)
                )
                .is_err()
            );
        }
    }
}
