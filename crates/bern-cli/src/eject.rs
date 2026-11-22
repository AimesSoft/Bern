//! `bern eject`: emit standalone native IDE projects that build and evolve
//! without `bern-cli`.

use crate::{
    Options, builds_here, context, doctor, eject_rust, find_workspace, framework, hosts_text,
    manifest, modules, print_notes, registry, report, run_steps, web_ui_page, write_if_changed,
};
use bern_target_api::{Diagnostic, EjectEngine, EjectOptions};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub fn run(options: &Options) -> ExitCode {
    let Some(manifest_path) = manifest(options) else {
        eprintln!(
            "error: no Bern.toml found; pass --manifest, or create a project with `bern new`"
        );
        return ExitCode::from(2);
    };
    let config =
        match bern_compiler::config::load_excluding(&manifest_path, &options.exclude_modules) {
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

    let Some(workspace) = find_workspace(&config.dir) else {
        eprintln!(
            "error: {} is not inside a Cargo workspace",
            config.dir.display()
        );
        return ExitCode::FAILURE;
    };
    let framework = match framework::detect(&workspace, &config.engine_package.name) {
        Ok(framework) => framework,
        Err(message) => {
            eprintln!("error: cannot tell where Bern comes from: {message}");
            return ExitCode::FAILURE;
        }
    };

    let engine = options.eject_engine.unwrap_or(EjectEngine::Source);
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
            manifest_path.display()
        );
        return ExitCode::FAILURE;
    }

    // In `--engine prebuilt` mode without `--target`, skip targets that cannot
    // prebuild a Rust engine binary on this host. In `--engine source` mode,
    // every target can be ejected cross-platform from any host OS.
    if options.target.is_none() && engine == EjectEngine::Prebuilt {
        let mut eligible = Vec::new();
        for (target_config, target) in targets {
            if !target.supports_prebuilt_eject() {
                eprintln!(
                    "   \x1b[1;33mSkipping\x1b[0m {}: Rust frontend requires `--engine source`",
                    target_config.name
                );
                continue;
            }
            if !builds_here(target) {
                eprintln!(
                    "   \x1b[1;33mSkipping\x1b[0m {}: `--engine prebuilt` builds on {} (use `--engine source` to eject cross-platform)",
                    target_config.name,
                    hosts_text(target)
                );
                continue;
            }
            if let Some(doctor::Check {
                title,
                status: doctor::Status::Missing(problem, _),
                ..
            }) = doctor::check(&target_config.platform)
            {
                eprintln!(
                    "   \x1b[1;33mSkipping\x1b[0m {}: `--engine prebuilt` needs {title}: {problem}",
                    target_config.name
                );
                continue;
            }
            eligible.push((target_config, target));
        }
        if eligible.is_empty() {
            eprintln!(
                "error: no target in {} can prebuild an engine on this host; pass `--engine source` to eject full native + Rust source projects",
                manifest_path.display()
            );
            return ExitCode::FAILURE;
        }
        targets = eligible;
    }

    let multiple = targets.len() > 1;
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    for (target_config, target) in targets {
        let descriptor = target.descriptor();
        if engine == EjectEngine::Prebuilt {
            if !target.supports_prebuilt_eject() {
                eprintln!(
                    "error: `{}` ({}) uses a Rust frontend and cannot use `--engine prebuilt`; pass `--engine source`",
                    target_config.name, descriptor.id
                );
                return ExitCode::FAILURE;
            }
            if !builds_here(target) {
                eprintln!(
                    "error: `{}` cannot prebuild its engine on this host (requires {}); pass `--engine source` to eject cross-platform",
                    target_config.name,
                    hosts_text(target)
                );
                return ExitCode::FAILURE;
            }
            if let Some(doctor::Check {
                title,
                status: doctor::Status::Missing(problem, fix),
                ..
            }) = doctor::check(&target_config.platform)
            {
                eprintln!(
                    "error: prebuilding `{}` needs {title}: {problem}",
                    target_config.name
                );
                for line in fix.lines() {
                    eprintln!("       {line}");
                }
                return ExitCode::FAILURE;
            }
        }

        let app = compiled.app.on_platform(&target_config.platform);
        let mut problems = target.check(&app, target_config);
        let features = bern_target_api::feature_report(&app, &target_config.platform);
        problems.extend(features.diagnostics.iter().cloned());
        report(&problems);
        if problems.iter().any(Diagnostic::is_error) {
            eprintln!(
                "error: `{}` cannot be ejected for `{}`",
                config.app_id, descriptor.id
            );
            return ExitCode::FAILURE;
        }

        let out_dir = match &options.out {
            Some(base) => {
                let resolved = if base.is_absolute() {
                    base.clone()
                } else {
                    cwd.join(base)
                };
                if multiple {
                    resolved.join(&target_config.name)
                } else {
                    resolved
                }
            }
            None => config.dir.join("ejected").join(&target_config.name),
        };

        if let Err(message) = prepare_output_dir(&out_dir, options.force) {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }

        let staging_dir = workspace
            .join("target/bern/eject-staging")
            .join(&target_config.name);
        let mut ctx = context(
            &config,
            &compiled,
            &workspace,
            target_config,
            &framework,
            options.release,
        );
        ctx.output_dir = out_dir.clone();

        let mut app = bern_target_api::style::for_platforms(&app, descriptor.platforms);
        if let Some(page) = &target_config.web_ui {
            match web_ui_page(
                &config,
                &compiled,
                &workspace,
                &registry,
                &framework,
                page,
                options.release,
            ) {
                Ok(files) => app.to_mut().resources.extend(files),
                Err(code) => return code,
            }
        }

        let eject_options = EjectOptions {
            engine,
            with_cli: options.with_cli,
            release: options.release,
            staging_dir: staging_dir.clone(),
        };

        let mut ejected = match target.eject(&app, &ctx, &eject_options) {
            Ok(ejected) => ejected,
            Err(errors) => {
                report(&errors);
                return ExitCode::FAILURE;
            }
        };

        if let Err(error) = modules::configure(&mut ejected.files, &config) {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }
        if !ejected.prebuilt_staging.is_empty()
            && let Err(error) = modules::configure(&mut ejected.prebuilt_staging, &config)
        {
            eprintln!("error: {error}");
            return ExitCode::FAILURE;
        }

        // Write ejected project files.
        for file in &ejected.files {
            let dest = out_dir.join(&file.path);
            if let Err(error) = write_if_changed(&dest, &file.contents) {
                eprintln!("error: cannot write {}: {error}", dest.display());
                return ExitCode::FAILURE;
            }
        }
        for rel in &ejected.executables {
            mark_executable(&out_dir.join(rel));
        }

        // Bundle the offline self-contained Rust workspace for Source mode
        // (and any optional `--with-cli` Rust entry manifests).
        let mut extra_notes = Vec::new();
        if !ejected.rust_manifests.is_empty() {
            match eject_rust::bundle_offline_workspace(
                &out_dir,
                &ejected.rust_root,
                &ejected.rust_manifests,
                &config,
                &workspace,
                &framework,
            ) {
                Ok(bundle) => {
                    eprintln!(
                        "   \x1b[1;32mVendored\x1b[0m {} app crate(s) and {} Bern runtime crate(s) offline",
                        bundle.app_crates, bundle.bern_crates
                    );
                    extra_notes.extend(bundle.notes);
                }
                Err(error) => {
                    eprintln!("error: cannot bundle Rust workspace: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }

        // Compile and copy prebuilt binaries in Prebuilt mode, then clean up
        // temporary staging glue files.
        if engine == EjectEngine::Prebuilt && !ejected.prebuilt_steps.is_empty() {
            let _ = std::fs::remove_dir_all(&staging_dir);
            for file in &ejected.prebuilt_staging {
                let dest = staging_dir.join(&file.path);
                if let Err(error) = write_if_changed(&dest, &file.contents) {
                    eprintln!("error: cannot write staging {}: {error}", dest.display());
                    return ExitCode::FAILURE;
                }
            }
            modules::environment(&mut ejected.prebuilt_steps, &config);
            let ok = run_steps(&ejected.prebuilt_steps);
            let _ = std::fs::remove_dir_all(&staging_dir);
            if !ok {
                return ExitCode::FAILURE;
            }
        }

        let mode_label = match engine {
            EjectEngine::Source => "source",
            EjectEngine::Prebuilt => "prebuilt",
        };
        eprintln!(
            "    \x1b[1;32mEjected\x1b[0m {} ({}, {mode_label}) into {}",
            target_config.name,
            descriptor.id,
            out_dir.display()
        );
        let mut notes = ejected.notes;
        notes.extend(extra_notes);
        print_notes(&notes);
    }

    ExitCode::SUCCESS
}

fn prepare_output_dir(out_dir: &Path, force: bool) -> Result<(), String> {
    if out_dir.exists() {
        let non_empty = std::fs::read_dir(out_dir)
            .map_err(|e| format!("cannot inspect {}: {e}", out_dir.display()))?
            .next()
            .is_some();
        if non_empty {
            if !force {
                return Err(format!(
                    "{} already exists and is not empty; pass `--force` to replace it or choose a new `--out` directory",
                    out_dir.display()
                ));
            }
            std::fs::remove_dir_all(out_dir)
                .map_err(|e| format!("cannot clear {}: {e}", out_dir.display()))?;
        }
    }
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))
}

fn mark_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(perms.mode() | 0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
