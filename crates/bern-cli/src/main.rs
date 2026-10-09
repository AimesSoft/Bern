//! `bern`: the build tool.
//!
//! It loads `Bern.toml`, compiles the declarations to IR, resolves each
//! target through the registry, lets the target check and generate, and
//! runs the target's build plan. The build tool is separate from the
//! application CLI that `bern build` generates.

use bern_compiler::{Compiled, Config};
use bern_target_api::{BuildContext, BuildStep, Diagnostic, Registry, Target};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "\
bern: build Bern applications

Usage:
  bern build [--manifest PATH] [--target NAME] [--release]
  bern check [--manifest PATH] [--target NAME]
  bern ir    [--manifest PATH]

  --manifest   Path to Bern.toml (default: search from the current directory)
  --target     Target name from [targets.<name>] (default: every target)
  --release    Optimized build
";

struct Options {
    command: String,
    manifest: Option<PathBuf>,
    target: Option<String>,
    release: bool,
}

fn parse() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| USAGE.to_string())?;
    if command == "--help" || command == "-h" || command == "help" {
        return Err(USAGE.to_string());
    }
    let mut options = Options {
        command,
        manifest: None,
        target: None,
        release: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => {
                options.manifest = Some(args.next().ok_or("--manifest needs a path")?.into())
            }
            "--target" => options.target = Some(args.next().ok_or("--target needs a name")?),
            "--release" => options.release = true,
            other => return Err(format!("unknown argument `{other}`\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register(Box::new(bern_backend_swiftui::SwiftUiMacos));
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

fn find_workspace(start: &Path) -> Option<PathBuf> {
    let mut directory = Some(start.to_path_buf());
    let mut found = None;
    while let Some(current) = directory {
        let manifest = current.join("Cargo.toml");
        if std::fs::read_to_string(&manifest)
            .is_ok_and(|text| text.lines().any(|line| line.trim() == "[workspace]"))
        {
            found = Some(current.clone());
        }
        directory = current.parent().map(Path::to_path_buf);
    }
    found
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
        BuildStep::Copy { from, to } => std::fs::copy(from, to).map(|_| ()).map_err(|error| {
            format!(
                "cannot copy {} to {}: {error}",
                from.display(),
                to.display()
            )
        }),
        BuildStep::RemoveIfExists { path } => {
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|error| error.to_string())
            } else if path.exists() {
                std::fs::remove_file(path).map_err(|error| error.to_string())
            } else {
                Ok(())
            }
        }
    }
}

fn context(
    config: &Config,
    compiled: &Compiled,
    workspace: &Path,
    target: &bern_target_api::TargetConfig,
    release: bool,
) -> BuildContext {
    BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: workspace.to_path_buf(),
        output_dir: workspace.join("target/bern").join(&target.name),
        target: target.clone(),
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        release,
    }
}

fn main() -> ExitCode {
    let options = match parse() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let Some(manifest) = options.manifest.clone().or_else(|| find_manifest(&cwd)) else {
        eprintln!("error: no Bern.toml found; pass --manifest");
        return ExitCode::from(2);
    };
    let config = match bern_compiler::config::load(&manifest) {
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
    if options.command != "build" && options.command != "check" {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let Some(workspace) = find_workspace(&config.dir) else {
        eprintln!(
            "error: {} is not inside a Cargo workspace",
            config.dir.display()
        );
        return ExitCode::FAILURE;
    };
    let registry = registry();
    let targets: Vec<_> = config
        .targets
        .iter()
        .filter(|target| {
            options
                .target
                .as_ref()
                .is_none_or(|name| *name == target.name)
        })
        .collect();
    if targets.is_empty() {
        eprintln!(
            "error: no matching [targets.*] entry in {}",
            manifest.display()
        );
        return ExitCode::FAILURE;
    }
    for target_config in targets {
        let target: &dyn Target = match registry.resolve(target_config) {
            Ok(target) => target,
            Err(error) => {
                report(&[error]);
                return ExitCode::FAILURE;
            }
        };
        let descriptor = target.descriptor();
        let problems = target.check(&compiled.app, target_config);
        report(&problems);
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
        if options.command == "check" {
            continue;
        }
        let context = context(
            &config,
            &compiled,
            &workspace,
            target_config,
            options.release,
        );
        let files = match target.generate(&compiled.app, &context) {
            Ok(files) => files,
            Err(errors) => {
                report(&errors);
                return ExitCode::FAILURE;
            }
        };
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
        let plan = match target.plan(&compiled.app, &context) {
            Ok(plan) => plan,
            Err(errors) => {
                report(&errors);
                return ExitCode::FAILURE;
            }
        };
        for step in &plan.steps {
            if let Err(message) = run_step(step) {
                eprintln!("error: {message}");
                return ExitCode::FAILURE;
            }
        }
        for product in &plan.products {
            eprintln!("   \x1b[1;32mFinished\x1b[0m {}", product.display());
        }
    }
    ExitCode::SUCCESS
}
