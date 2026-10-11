//! Generate the showcase for `winui3 + windows` and, on Windows with .NET
//! installed, compile the generated application, so generator mistakes
//! fail here rather than in `bern build`.

use bern_backend_winui3::{WinUi3Windows, product_name};
use bern_ir::Application;
use bern_target_api::{BuildContext, BuildStep, Registry, Target, TargetConfig};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    // Without the verbatim prefix Windows gives canonical paths.
    match root.to_string_lossy().strip_prefix(r"\\?\") {
        Some(plain) => PathBuf::from(plain),
        None => root,
    }
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

fn windows_target(config: &bern_compiler::Config) -> TargetConfig {
    config
        .targets
        .iter()
        .find(|target| target.platform == "windows")
        .expect("the showcase declares a Windows target")
        .clone()
}

#[test]
fn windows_resolves_to_winui3_by_default() {
    let mut registry = Registry::new();
    registry.register(Box::new(WinUi3Windows));
    let config = TargetConfig {
        name: "windows".into(),
        platform: "windows".into(),
        ..TargetConfig::default()
    };
    assert_eq!(
        registry.resolve(&config).unwrap().descriptor().id,
        "winui3-windows"
    );
}

#[test]
fn languages_without_runtime_texts_are_rejected() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    app.languages.supported.push("ko".into());
    let problems = WinUi3Windows.check(&app, &windows_target(&config));
    assert!(
        problems
            .iter()
            .any(|problem| problem.message.contains("no `ko` translation")),
        "{problems:?}"
    );
}

#[test]
fn showcase_generates_a_winui3_application() {
    let root = workspace();
    let (config, compiled) = showcase();
    let target_config = windows_target(&config);
    let problems = WinUi3Windows.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(product_name(&compiled.app), "Showcase");
    let output = root.join("target/bern-test/winui3");
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
        release: false,
    };
    let files = WinUi3Windows.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }

    // The engine library learns the declared resource names and the
    // channel, and exports the start function.
    let lib = std::fs::read_to_string(output.join("glue/src/lib.rs")).unwrap();
    assert!(
        lib.contains(
            r#"resources: vec!["media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{lib}"
    );
    assert!(lib.contains("ipc: true"), "{lib}");
    assert!(
        lib.contains("pub unsafe extern \"C\" fn bern_app_start("),
        "{lib}"
    );
    let cargo = std::fs::read_to_string(output.join("glue/Cargo.toml")).unwrap();
    assert!(cargo.contains(r#"crate-type = ["cdylib"]"#), "{cargo}");
    assert!(
        !cargo.contains(r"\\?\") && !cargo.contains("//?/"),
        "{cargo}"
    );
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    for expected in [
        r#""ui": "winui3""#,
        r#""backend": "windows-mediaplayer""#,
        r#""media/tides.mp4""#,
    ] {
        assert!(manifest.contains(expected), "{manifest}");
    }
    let project = std::fs::read_to_string(output.join("app/Showcase.csproj")).unwrap();
    for expected in [
        "<UseWinUI>true</UseWinUI>",
        "<WindowsPackageType>None</WindowsPackageType>",
        "<AssemblyName>showcase</AssemblyName>",
        "<Product>Bern 控件展示</Product>",
        "Link=\"bern_app.dll\"",
    ] {
        assert!(project.contains(expected), "missing `{expected}`");
    }
    for runtime in [
        "Bern/BernCore.cs",
        "Bern/BernMedia.cs",
        "Bern/BernStructure.cs",
    ] {
        assert!(output.join("app").join(runtime).is_file(), "{runtime}");
    }

    // Every component is realized; colors come from the system theme.
    let app = std::fs::read_to_string(output.join("app/App.cs")).unwrap();
    assert!(!app.contains("unsupported"), "{app}");
    for forbidden in ["Color.FromArgb", "Colors.", "SolidColorBrush"] {
        assert!(
            !app.contains(forbidden),
            "generated code must not use `{forbidden}`"
        );
    }
    for expected in [
        "B.SplitView(p, c, \"pages\", new Field(0, 0)",
        "B.Navigation(p, c, \"library\"",
        "B.Query(p, c, \"library_results\"",
        "B.Adaptive(p, c, \"clip_choice\",",
        "B.Tabs(p, c, \"tabs\"",
        "B.Video(p, c, \"preview\", source: null, player: \"preview\"",
        "static CloseCheck close_settings(Ctx c) => new()",
        "Launcher.Run(Ui.App, \"dev.bern.showcase\")",
    ] {
        assert!(app.contains(expected), "missing `{expected}`");
    }

    // The plan builds the engine library, then the application, ships the
    // resources next to the executable and copies the entries.
    let plan = WinUi3Windows.plan(&compiled.app, &context).unwrap();
    let programs: Vec<&str> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            BuildStep::Run { program, .. } => Some(program.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(programs, ["cargo", "dotnet"]);
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
            output.join("Showcase/showcase.exe"),
            output.join("bin/showcase.exe"),
            output.join("bin/showcase-mcp.exe"),
        ]
    );

    compile(&output);
}

/// Compile the generated application on Windows where .NET is installed.
#[cfg(windows)]
fn compile(output: &Path) {
    use std::process::Command;
    let dotnet = Command::new("dotnet")
        .arg("--version")
        .output()
        .is_ok_and(|result| result.status.success());
    if !dotnet {
        eprintln!(".NET not installed; skipping the compile check");
        return;
    }
    let result = Command::new("dotnet")
        .arg("build")
        .arg(output.join("app/Showcase.csproj"))
        .arg("--output")
        .arg(output.join("compiled"))
        .arg("--nologo")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "the C# compiler rejected the generated application:\n{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[cfg(not(windows))]
fn compile(_: &Path) {
    eprintln!("not on Windows; skipping the compile check");
}
