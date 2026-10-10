//! Generate the showcase and type-check the Swift output with the installed
//! toolchain, so generator mistakes fail here rather than in `bern build`.

use bern_backend_swiftui::{MINIMUM_MACOS, SwiftUiMacos};
use bern_target_api::{BuildContext, Target, TargetConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn showcase_generates_swift_that_type_checks() {
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
