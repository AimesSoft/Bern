//! Generate the showcase for `gtk + linux` and, where GTK 4 and
//! libadwaita are installed, type-check the generated application, so
//! generator mistakes fail here rather than in `bern build`.

use bern_backend_gtk::{GtkLinux, product_name};
use bern_ir::Application;
use bern_target_api::{BuildContext, BuildStep, Framework, Registry, Target, TargetConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn showcase() -> (bern_compiler::Config, bern_compiler::Compiled) {
    let root = workspace();
    let config = bern_compiler::config::load(&root.join("examples/showcase/Bern.toml")).unwrap();
    let compiled = bern_compiler::compile(&config).unwrap_or_else(|errors| {
        panic!(
            "{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    (config, compiled)
}

fn linux_target(config: &bern_compiler::Config) -> TargetConfig {
    config
        .targets
        .iter()
        .find(|target| target.platform == "linux")
        .expect("the showcase declares a Linux target")
        .clone()
}

#[test]
fn linux_resolves_to_gtk_by_default() {
    let mut registry = Registry::new();
    registry.register(Box::new(GtkLinux));
    let config = TargetConfig {
        name: "linux".into(),
        platform: "linux".into(),
        ..TargetConfig::default()
    };
    assert_eq!(
        registry.resolve(&config).unwrap().descriptor().id,
        "gtk-linux"
    );
    let explicit = TargetConfig {
        ui: Some("flutter".into()),
        ..config
    };
    let error = registry.resolve(&explicit).err().unwrap();
    assert!(
        error.message.contains("flutter + linux"),
        "{}",
        error.message
    );
}

#[test]
fn languages_without_runtime_texts_are_rejected() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    app.languages.supported.push("ko".into());
    let problems = GtkLinux.check(&app, &linux_target(&config));
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("no `ko` translation")),
        "{problems:?}"
    );
}

#[test]
fn showcase_generates_a_gtk_application() {
    let root = workspace();
    let (config, compiled) = showcase();
    let target_config = linux_target(&config);
    let problems = GtkLinux.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(product_name(&compiled.app), "Showcase");
    let output = root.join("target/bern-test/gtk");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: target_config,
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    let files = GtkLinux.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }

    // The engine learns the declared resource names and the channel.
    let main = std::fs::read_to_string(output.join("app/src/main.rs")).unwrap();
    assert!(
        main.contains(
            r#"resources: vec!["media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{main}"
    );
    assert!(main.contains("ipc: true"), "{main}");
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    for expected in [
        r#""ui": "gtk""#,
        r#""backend": "gtk-mediafile""#,
        r#""media/tides.mp4""#,
    ] {
        assert!(manifest.contains(expected), "{manifest}");
    }
    let desktop =
        std::fs::read_to_string(output.join("Showcase/dev.bern.showcase.desktop")).unwrap();
    assert!(desktop.contains("Name[zh_CN]=Bern 控件展示"), "{desktop}");
    // Without `[app].icon` the Bern logo sits beside the executable, named
    // after the application, and the desktop entry shows it.
    let icon = output.join("Showcase/icons/dev.bern.showcase.png");
    let png = std::fs::read(&icon).unwrap();
    assert_eq!(bern_target_api::icon::png_size(&png), Some((512, 512)));
    // Desktop entries write paths with `/`, also when generated on Windows.
    let written = icon.display().to_string().replace('\\', "/");
    assert!(desktop.contains(&format!("Icon={written}\n")), "{desktop}");

    // Every component is realized; colors come from the system style.
    let ui = std::fs::read_to_string(output.join("app/src/ui.rs")).unwrap();
    assert!(!ui.contains("unsupported"), "{ui}");
    for forbidden in ["CssProvider", "rgb(", "@define-color", "gdk::RGBA"] {
        assert!(
            !ui.contains(forbidden),
            "generated code must not use `{forbidden}`"
        );
    }
    for expected in [
        "b::SplitView(p, c, b::SplitSpec {",
        "b::Navigation(p, c, Some(\"library\")",
        "b::Query(p, c, b::QueryProps { id: Some(\"library_results\")",
        "b::Adaptive(p, c, Some(\"clip_choice\")",
        "b::Tabs(p, c, Some(\"tabs\")",
        "b::Video(p, c, b::VideoProps { id: \"preview\", source: None, player: Some(\"preview\")",
        "fn close_settings(c: &b::Ctx) -> b::CloseCheck",
    ] {
        assert!(ui.contains(expected), "missing `{expected}`");
    }

    // The plan builds the application, ships the resources next to the
    // executable and copies the entries.
    let plan = GtkLinux.plan(&compiled.app, &context).unwrap();
    let BuildStep::Run { program, .. } = &plan.steps[0] else {
        panic!("the plan starts with cargo");
    };
    assert_eq!(program, "cargo");
    let mirror = plan
        .steps
        .iter()
        .find_map(|step| match step {
            BuildStep::Mirror { dir, files, .. } => Some((dir, files)),
            _ => None,
        })
        .expect("a mirror step");
    assert_eq!(mirror.0, &output.join("Showcase/resources"));
    assert_eq!(
        mirror.1[0],
        (
            config.dir.join("media/aurora.mp4"),
            PathBuf::from("media/aurora.mp4")
        )
    );
    assert_eq!(
        plan.products,
        [
            output.join("Showcase/showcase"),
            output.join("bin/showcase"),
            output.join("bin/showcase-mcp"),
        ]
    );

    // Type-check the generated application where GTK is installed.
    let gtk = Command::new("pkg-config")
        .args(["--exists", "gtk4 >= 4.14", "libadwaita-1 >= 1.5"])
        .status()
        .is_ok_and(|status| status.success());
    if !gtk {
        eprintln!("GTK 4.14 and libadwaita 1.5 not installed; skipping the type check");
        return;
    }
    let result = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(output.join("app/Cargo.toml"))
        .arg("--target-dir")
        .arg(root.join("target/bern-glue-linux"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "rustc rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
