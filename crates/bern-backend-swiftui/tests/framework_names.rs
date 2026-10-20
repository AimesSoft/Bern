//! The `framework-names` example names its value types like types of
//! SwiftUI, Foundation and Swift (`Layout`, `View`, `Text`, `Color`,
//! `Error`, `Task`…). Generated Swift spells them `App_…`, so they hide
//! none of the frameworks' types; swiftc type-checks the result for
//! macOS, iOS and tvOS where the SDKs are installed.

use bern_backend_swiftui::{
    MINIMUM_IOS, MINIMUM_MACOS, MINIMUM_TVOS, SwiftUiIos, SwiftUiMacos, SwiftUiTvos,
};
use bern_target_api::{BuildContext, Framework, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Generate the example for `platform` into
/// `target/bern-test/swiftui-names-<platform>`.
fn generate(platform: &str, target: &dyn Target) -> PathBuf {
    let root = workspace();
    let config =
        bern_compiler::config::load(&root.join("examples/framework-names/Bern.toml")).unwrap();
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
    let target_config = config
        .targets
        .iter()
        .find(|target| target.platform == platform)
        .unwrap()
        .clone();
    let problems = target.check(&compiled.app, &target_config);
    assert!(problems.is_empty(), "{platform}: {problems:?}");
    let output = root.join(format!("target/bern-test/swiftui-names-{platform}"));
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
        modules: Vec::new(),
        framework: Framework::Path(root.join("crates")),
        release: false,
    };
    for file in target.generate(&compiled.app, &context).unwrap() {
        let path = output.join(&file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.contents).unwrap();
    }
    output
}

/// The value types are declared and used under their `App_` names only.
fn check_names(output: &Path) {
    let swift = std::fs::read_to_string(output.join("Sources/BernGenerated.swift")).unwrap();
    for expected in [
        "enum App_Layout: Hashable, Sendable, BernCodable, CaseIterable {",
        "extension App_Layout: BernLabeled {",
        "enum App_View: Hashable, Sendable, BernCodable, CaseIterable {",
        "enum App_Error: Hashable, Sendable, BernCodable, CaseIterable {",
        "enum App_Task: Hashable, Sendable, BernCodable, CaseIterable {",
        "struct App_Text: Hashable, Sendable {",
        "var texts: [App_Text]",
        "case text(text: App_Text)",
        "case list(items: [String])",
        "enum App_Shape: Hashable, Sendable, BernCodable, CaseIterable {",
        "private(set) var layout: App_Layout = App_Layout.column",
        "private(set) var error: Optional<App_Error> = nil",
        "bernEngine.paint(App_Color.green, from: bernWindow)",
        r#"bernEngine.show(App_Content.string(value: "words"), from: bernWindow)"#,
        "let text: App_Text",
        // Errors name the declared type.
        r#"throw BernDecodeError(message: "invalid Layout")"#,
    ] {
        assert!(swift.contains(expected), "BernGenerated.swift lacks `{expected}`");
    }
    for bare in [
        "enum Layout", "enum View", "enum Color", "enum Error", "struct Text", "struct List",
    ] {
        assert!(
            !swift.contains(&format!("\n{bare}")),
            "BernGenerated.swift declares `{bare}`"
        );
    }
}

enum Apple {
    Macos,
    Ios,
    Tvos,
}

/// Type-check generated Swift against the SDK of `system`, when it is
/// installed.
fn typecheck(output: &Path, system: Apple) {
    let architecture = if std::env::consts::ARCH == "x86_64" {
        "x86_64"
    } else {
        "arm64"
    };
    let (sdk, target, platform) = match system {
        Apple::Macos => (
            "macosx",
            format!("{architecture}-apple-macos{MINIMUM_MACOS}"),
            "BernAppKit.swift",
        ),
        Apple::Ios => (
            "iphonesimulator",
            format!("{architecture}-apple-ios{MINIMUM_IOS}-simulator"),
            "BernUIKit.swift",
        ),
        Apple::Tvos => (
            "appletvsimulator",
            format!("{architecture}-apple-tvos{MINIMUM_TVOS}-simulator"),
            "BernUIKit.swift",
        ),
    };
    let Some(sdk) = Command::new("xcrun")
        .args(["--sdk", sdk, "--show-sdk-path"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    else {
        eprintln!("no {sdk} SDK; skipping type check");
        return;
    };
    let sources = output.join("Sources");
    let result = Command::new("xcrun")
        .args([
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-parse-as-library",
            "-sdk",
        ])
        .arg(sdk)
        .arg("-target")
        .arg(target)
        .arg("-module-cache-path")
        .arg(workspace().join("target/bern-swift-module-cache"))
        .arg("-import-objc-header")
        .arg(sources.join("bern.h"))
        .arg(sources.join("BernRuntime.swift"))
        .arg(sources.join("BernMedia.swift"))
        .arg(sources.join("BernStyle.swift"))
        .arg(sources.join(platform))
        .arg(sources.join("BernGenerated.swift"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "swiftc rejected the generated code:\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn value_types_named_like_framework_types_compile_on_macos() {
    let output = generate("macos", &SwiftUiMacos);
    check_names(&output);
    typecheck(&output, Apple::Macos);
}

#[test]
fn value_types_named_like_framework_types_compile_on_ios() {
    let output = generate("ios", &SwiftUiIos);
    check_names(&output);
    typecheck(&output, Apple::Ios);
}

#[test]
fn value_types_named_like_framework_types_compile_on_tvos() {
    let output = generate("tvos", &SwiftUiTvos);
    check_names(&output);
    typecheck(&output, Apple::Tvos);
}
