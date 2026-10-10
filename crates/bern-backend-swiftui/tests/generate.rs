//! Generate the showcase and type-check the Swift output with the installed
//! toolchain, so generator mistakes fail here rather than in `bern build`.

use bern_backend_swiftui::{MINIMUM_MACOS, SwiftUiMacos};
use bern_ir::{Application, ResourceDef, SourceLocation};
use bern_target_api::{BuildContext, BuildStep, Target, TargetConfig};
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

#[test]
fn resources_must_fit_the_bundle_layout() {
    let (config, compiled) = showcase();
    let mut app: Application = compiled.app;
    let declared = SourceLocation {
        file: "Bern.toml".into(),
        line: 40,
        column: 10,
    };
    for name in [
        "en.lproj/Localizable.strings",
        "notes.lproj",
        "v1.2/clip.mp4",
        "media/clip.v2.mp4",
    ] {
        app.resources.push(ResourceDef {
            name: name.into(),
            declared: declared.clone(),
        });
    }
    let problems = SwiftUiMacos.check(&app, &config.targets[0]);
    let messages: Vec<&str> = problems.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(problems.len(), 3, "{messages:?}");
    assert!(messages[0].contains("`en.lproj` is where the macOS bundle keeps localizations"));
    assert!(messages[1].contains("`notes.lproj`"));
    assert!(messages[2].contains("directory `v1.2` contains `.`"));
    assert!(
        problems
            .iter()
            .all(|d| d.location == Some(declared.clone()))
    );
}

#[test]
fn showcase_generates_swift_that_type_checks() {
    let root = workspace();
    let (config, compiled) = showcase();
    let names: Vec<&str> = compiled
        .app
        .resources
        .iter()
        .map(|resource| resource.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["media/aurora.mp4", "media/broken.mp4", "media/tides.mp4"]
    );
    let target_config = config.targets[0].clone();
    let problems = SwiftUiMacos.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{problems:?}");
    let output = root.join("target/bern-test/swiftui");
    let _ = std::fs::remove_dir_all(&output);
    let context = BuildContext {
        project_dir: config.dir.clone(),
        workspace_dir: root.clone(),
        output_dir: output.clone(),
        target: TargetConfig { ..target_config },
        protocol_package: config.protocol_package.name.clone(),
        protocol_path: config.protocol_package.dir.clone(),
        protocol_module: compiled.protocol_module.clone(),
        engine_package: config.engine_package.name.clone(),
        engine_path: config.engine_package.dir.clone(),
        engine_type: config.engine_type.clone(),
        release: false,
    };
    let files = SwiftUiMacos.generate(&compiled.app, &context).unwrap();
    for file in &files {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    // The engine learns the declared names; the plan ships them into the
    // bundle next to the generated localizations, before signing.
    let glue = std::fs::read_to_string(output.join("glue/src/lib.rs")).unwrap();
    assert!(
        glue.contains(
            r#"resources: vec!["media/aurora.mp4".into(), "media/broken.mp4".into(), "media/tides.mp4".into()]"#
        ),
        "{glue}"
    );
    let manifest = std::fs::read_to_string(output.join("build.json")).unwrap();
    assert!(manifest.contains(r#""media/tides.mp4""#), "{manifest}");
    let plan = SwiftUiMacos.plan(&compiled.app, &context).unwrap();
    let mirror = plan
        .steps
        .iter()
        .position(|step| matches!(step, BuildStep::Mirror { .. }))
        .expect("a mirror step");
    let sign = plan
        .steps
        .iter()
        .position(|step| matches!(step, BuildStep::Run { program, .. } if program == "codesign"))
        .unwrap();
    assert!(mirror < sign);
    let BuildStep::Mirror { dir, files, keep } = &plan.steps[mirror] else {
        unreachable!()
    };
    assert_eq!(dir, &output.join("Showcase.app/Contents/Resources"));
    assert_eq!(
        files[0],
        (
            config.dir.join("media/aurora.mp4"),
            PathBuf::from("media/aurora.mp4")
        )
    );
    assert!(keep.contains(&PathBuf::from("en.lproj/InfoPlist.strings")));
    assert!(keep.contains(&PathBuf::from("zh-Hans.lproj/Localizable.strings")));

    let generated = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for forbidden in [
        "Color(",
        ".background(",
        "glassEffect",
        "#available",
        "WKWebView",
        "JSONDecoder",
    ] {
        assert!(
            !generated.contains(forbidden),
            "generated Swift must not use `{forbidden}`"
        );
    }
    if !Command::new("xcrun")
        .arg("--find")
        .arg("swiftc")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        eprintln!("swiftc not available; skipping type check");
        return;
    }
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
        ])
        .arg("-target")
        .arg(format!("arm64-apple-macos{MINIMUM_MACOS}"))
        .arg("-module-cache-path")
        .arg(root.join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
